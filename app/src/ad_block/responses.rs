//! uBlock Origin's response-time filters (#2758), which adblock-rust doesn't read: `header=`, which
//! blocks by a response header's value, and `ipaddress=`, by the server's address. They're parsed
//! and matched here as uBlock Origin 1.75 does on Chromium.
//!
//! Chromium tells uBlock Origin no server address as a request starts or at its headers, so its
//! `ipaddress=` matches only a host written as an IP address, as the request starts. This does the
//! same, at the request: don't "fix" it to use the server's real address, which would block what
//! uBlock Origin on Chromium never does. `header=` waits for the response, which the core asks
//! about only for a request whose address one of them matched as it started, with the values of the
//! headers they name and nothing else.

use std::collections::HashMap;

use fancy_regex::Regex;

/// Why a filter of these isn't honoured, for one the app doesn't read in full.
pub type NotHonoured = String;

/// The options uBlock Origin 1.75 knows, its netOptionTokenDescriptors, by which it finds where a
/// filter's options start.
const OPTION_NAMES: &[&str] = &[
    "1p",
    "first-party",
    "strict1p",
    "strict-first-party",
    "3p",
    "third-party",
    "strict3p",
    "strict-third-party",
    "all",
    "badfilter",
    "cname",
    "csp",
    "css",
    "stylesheet",
    "denyallow",
    "doc",
    "document",
    "ehide",
    "elemhide",
    "empty",
    "frame",
    "subdocument",
    "from",
    "domain",
    "font",
    "genericblock",
    "ghide",
    "generichide",
    "image",
    "important",
    "inline-font",
    "inline-script",
    "ipaddress",
    "match-case",
    "media",
    "method",
    "mp4",
    "_",
    "object",
    "object-subrequest",
    "other",
    "permissions",
    "ping",
    "beacon",
    "popunder",
    "popup",
    "reason",
    "redirect",
    "rewrite",
    "redirect-rule",
    "removeparam",
    "queryprune",
    "replace",
    "requestheader",
    "responseheader",
    "header",
    "script",
    "shide",
    "specifichide",
    "to",
    "top",
    "urlskip",
    "uritransform",
    "xhr",
    "xmlhttprequest",
    "webrtc",
    "websocket",
];

/// The request types options name, each as the core names it.
const TYPES: &[(&str, &str)] = &[
    ("script", "script"),
    ("xhr", "xmlhttprequest"),
    ("xmlhttprequest", "xmlhttprequest"),
    ("frame", "subdocument"),
    ("subdocument", "subdocument"),
    ("doc", "document"),
    ("document", "document"),
    ("css", "stylesheet"),
    ("stylesheet", "stylesheet"),
    ("image", "image"),
    ("media", "media"),
    ("font", "font"),
    ("object", "object"),
    ("ping", "ping"),
    ("beacon", "ping"),
    ("websocket", "websocket"),
    ("other", "other"),
];

/// Where `line`'s options start, as uBlock Origin's indexOfNetAnchor finds it: the last `$` an
/// option's name follows, as a regular expression's own `$` can come later.
fn options_at(line: &str) -> Option<usize> {
    let mut end = line.len();
    while let Some(at) = line[..end].rfind('$') {
        if starts_with_option(&line[at + 1..]) {
            return Some(at);
        }
        end = at;
    }
    None
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Whether `text` starts with an option's name, `~` before it or not, as a word.
fn starts_with_option(text: &str) -> bool {
    let text = text.strip_prefix('~').unwrap_or(text);
    OPTION_NAMES
        .iter()
        .any(|name| (text.strip_prefix(name)).is_some_and(|rest| !rest.starts_with(is_word)))
}

/// Whether an option starts at `text`, just after a comma, as uBlock Origin's reNetOptionComma
/// takes one: a name of `~`, `1`, `3`, `a`–`z` and `-`, then `=` or the next comma or the end; or
/// underscores alone. A comma in a value, `{2,}` or an escaped `\,`, is followed by neither.
fn option_follows(text: &str) -> bool {
    let underscores = text.len() - text.trim_start_matches('_').len();
    if underscores > 0 {
        let rest = &text[underscores..];
        return rest.is_empty() || rest.starts_with(',');
    }
    let name = text.strip_prefix('~').unwrap_or(text);
    let length = name.len()
        - name
            .trim_start_matches(|c: char| matches!(c, '1' | '3' | 'a'..='z' | '-'))
            .len();
    let rest = &name[length..];
    length > 0 && (rest.is_empty() || rest.starts_with([',', '=']))
}

/// The options in `text`, after the anchor's `$`, each `~name` or `name=value`, whose value runs
/// to the next comma an option follows.
fn options(text: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut start = 0;
    while start <= text.len() {
        let rest = &text[start..];
        let end = (rest.match_indices(','))
            .find(|(at, _)| option_follows(&rest[at + 1..]))
            .map_or(rest.len(), |(at, _)| at);
        found.push(&rest[..end]);
        start += end + 1;
    }
    found
}

/// Domains an option names, `a.com|~b.com`: where none it names but a `~` one is, any.
#[derive(Default)]
struct Domains {
    yes: Vec<String>,
    no: Vec<String>,
}

impl Domains {
    fn parse(value: &str) -> Result<Self, NotHonoured> {
        let mut domains = Self::default();
        for entry in value.split('|') {
            let (not, entry) = match entry.strip_prefix('~') {
                Some(entry) => (true, entry),
                None => (false, entry),
            };
            if entry.is_empty() || entry.ends_with(".*") || entry.starts_with('/') {
                return Err(format!("its domain {entry:?} isn't one the app reads"));
            }
            // A host's IPv6 address is compared without its brackets, as host_of gives it.
            let entry = entry
                .trim_start_matches('[')
                .trim_end_matches(']')
                .to_ascii_lowercase();
            match not {
                true => domains.no.push(entry),
                false => domains.yes.push(entry),
            }
        }
        Ok(domains)
    }

    fn matches(&self, host: &str) -> bool {
        let under = |entry: &String| {
            host == entry
                || (host.strip_suffix(entry.as_str())).is_some_and(|start| start.ends_with('.'))
        };
        !self.no.iter().any(under) && (self.yes.is_empty() || self.yes.iter().any(under))
    }
}

/// What a filter tests that adblock-rust's don't.
enum Test {
    /// A response header: its first of `name`, present, or with a value `value` matches, or with
    /// `not`, the opposite, as uBlock Origin's parseHeaderValue reads them.
    Header {
        name: String,
        not: bool,
        value: Option<Regex>,
    },
    /// The host, written as an IP address.
    Address(Regex),
}

/// One of the filters.
struct Filter {
    exception: bool,
    important: bool,
    /// None for any address.
    url: Option<Regex>,
    /// The types it's for, as the core names them; None for each but a page's own.
    types: Option<Vec<&'static str>>,
    not_types: Vec<&'static str>,
    first_party: Option<bool>,
    same_host: Option<bool>,
    from: Domains,
    to: Domains,
    /// Lowercase, each with whether it's negated; none for any method.
    methods: Vec<(bool, String)>,
    test: Test,
    /// The stand-in's name a `$redirect` names, which answers a request it blocks.
    redirect: Option<String>,
}

/// The regular expression of a filter's address pattern that isn't one already, as uBlock Origin
/// matches it: `||` at a host or a label of it, `|` at an end, `*` anything, `^` a separator.
fn pattern_regex(pattern: &str) -> String {
    let mut out = String::new();
    let mut rest = pattern;
    if let Some(after) = rest.strip_prefix("||") {
        out.push_str(r"^[a-z][a-z0-9+.-]*://(?:[^/?#]*\.)?");
        rest = after;
    } else if let Some(after) = rest.strip_prefix('|') {
        out.push('^');
        rest = after;
    }
    let (rest, end) = match rest.strip_suffix('|') {
        Some(rest) => (rest, "$"),
        None => (rest, ""),
    };
    for c in rest.chars() {
        match c {
            '*' => out.push_str(".*"),
            '^' => out.push_str("(?:[^0-9A-Za-z_.%-]|$)"),
            c => out.push_str(&fancy_regex::escape(&c.to_string())),
        }
    }
    out.push_str(end);
    out
}

fn regex(pattern: &str, case_insensitive: bool) -> Result<Regex, NotHonoured> {
    let flags = if case_insensitive { "(?i)" } else { "" };
    Regex::new(&format!("{flags}{pattern}"))
        .map_err(|error| format!("its regular expression doesn't compile: {error}"))
}

/// The test of `header=`'s value, as uBlock Origin's parseHeaderValue reads it: a name, then after
/// `:` an optional `~`, and a `/regular expression/`, one with `/i`, or text to find with `*` and `?`
/// wildcards, in any case. With no value, the header being there.
fn header_test(arg: &str) -> Result<Test, NotHonoured> {
    let arg = arg.trim();
    let (name, rest) = arg.split_once(':').unwrap_or((arg, ""));
    if name.is_empty() {
        return Err("its header= names no header".into());
    }
    let (not, value) = match rest.strip_prefix('~') {
        Some(value) => (true, value),
        None => (false, rest),
    };
    let value = match value {
        "" => None,
        value => Some(
            match value.strip_prefix('/').and_then(|v| v.rsplit_once('/')) {
                Some((pattern, flags @ ("" | "i"))) if !pattern.is_empty() => {
                    regex(pattern, flags == "i")?
                }
                _ => {
                    let mut pattern = String::new();
                    let mut escaped = false;
                    for c in value.chars() {
                        match c {
                            '*' if !escaped => pattern.push_str(".*"),
                            '?' if !escaped => pattern.push_str(".?"),
                            c => pattern.push_str(&fancy_regex::escape(&c.to_string())),
                        }
                        escaped = c == '\\';
                    }
                    regex(&pattern, true)?
                }
            },
        ),
    };
    Ok(Test::Header {
        name: name.to_ascii_lowercase(),
        not,
        value,
    })
}

/// The test of `ipaddress=`'s value, as uBlock Origin's FilterIPAddress reads it: a
/// `/regular expression/`, an address ending in `*` for those it starts, or one address.
fn address_test(value: &str) -> Result<Test, NotHonoured> {
    let pattern = match value {
        "lan" | "loopback" => return Err(format!("ipaddress={value} isn't one the app reads")),
        value if value.len() > 2 && value.starts_with('/') && value.ends_with('/') => {
            value[1..value.len() - 1].to_string()
        }
        value => match value.strip_suffix('*') {
            Some(start) => format!("^{}", fancy_regex::escape(start)),
            None => format!("^{}$", fancy_regex::escape(value)),
        },
    };
    Ok(Test::Address(regex(&pattern, false)?))
}

impl Filter {
    /// `line` as one of these filters, or None for one of none of their options.
    fn parse(line: &str) -> Result<Option<Self>, NotHonoured> {
        let Some(at) = options_at(line) else {
            return Ok(None);
        };
        let options = options(&line[at + 1..]);
        let named = |name: &str| {
            options.iter().any(|option| {
                option
                    .split_once('=')
                    .is_some_and(|(option, _)| option == name)
            })
        };
        if !named("header") && !named("ipaddress") && !named("requestheader") {
            return Ok(None);
        }
        let (exception, pattern) = match line[..at].strip_prefix("@@") {
            Some(pattern) => (true, pattern),
            None => (false, &line[..at]),
        };
        let mut filter = Filter {
            exception,
            important: false,
            url: None,
            types: None,
            not_types: Vec::new(),
            first_party: None,
            same_host: None,
            from: Domains::default(),
            to: Domains::default(),
            methods: Vec::new(),
            test: Test::Header {
                name: String::new(),
                not: false,
                value: None,
            },
            redirect: None,
        };
        let mut match_case = false;
        let mut test = None;
        for option in &options {
            let (name, value) = match option.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (*option, None),
            };
            let (not, bare) = match name.strip_prefix('~') {
                Some(bare) => (true, bare),
                None => (false, name),
            };
            match (bare, value) {
                ("header", Some(value)) => test = Some(header_test(value)?),
                ("ipaddress", Some(value)) => test = Some(address_test(value)?),
                ("requestheader", _) => {
                    return Err(
                        "requestheader= tests the request's own headers, Cookie among \
                                them, which the core doesn't hand over (orchestrator, 2026-11-14)"
                            .into(),
                    );
                }
                ("replace", _) => {
                    return Err(
                        "replace= rewrites the response's body, which uBlock Origin on \
                                Chromium doesn't do either"
                            .into(),
                    );
                }
                ("match-case", None) => match_case = true,
                ("important", None) => filter.important = true,
                ("1p" | "first-party", None) => filter.first_party = Some(!not),
                ("3p" | "third-party", None) => filter.first_party = Some(not),
                ("strict1p" | "strict-first-party", None) => filter.same_host = Some(true),
                ("strict3p" | "strict-third-party", None) => filter.same_host = Some(false),
                ("from" | "domain", Some(value)) => filter.from = Domains::parse(value)?,
                ("to", Some(value)) => filter.to = Domains::parse(value)?,
                ("method", Some(value)) => {
                    filter.methods = (value.split('|'))
                        .map(|method| match method.strip_prefix('~') {
                            Some(method) => (true, method.to_ascii_lowercase()),
                            None => (false, method.to_ascii_lowercase()),
                        })
                        .collect();
                }
                ("redirect" | "redirect-rule", Some(value)) => {
                    let name = value.split(':').next().unwrap_or(value);
                    filter.redirect = Some(name.to_string());
                }
                ("all", None) => {
                    let types = filter.types.get_or_insert_with(Vec::new);
                    types.extend(TYPES.iter().map(|(_, kind)| *kind));
                }
                ("_", None) => {}
                (bare, None) if TYPES.iter().any(|(name, _)| *name == bare) => {
                    let kind = TYPES.iter().find(|(name, _)| *name == bare).unwrap().1;
                    match not {
                        true => filter.not_types.push(kind),
                        false => filter.types.get_or_insert_with(Vec::new).push(kind),
                    }
                }
                _ => return Err(format!("its option {name} isn't one the app reads")),
            }
        }
        filter.test = test.expect("named one of them");
        filter.url = match pattern {
            "" | "*" => None,
            pattern if pattern.len() > 2 && pattern.starts_with('/') && pattern.ends_with('/') => {
                Some(regex(&pattern[1..pattern.len() - 1], !match_case)?)
            }
            pattern => Some(regex(&pattern_regex(pattern), !match_case)?),
        };
        Ok(Some(filter))
    }

    /// Whether it's for a request of `asked`, but for its method and its test.
    fn is_for(&self, asked: &Asked) -> bool {
        let kind_fits = match &self.types {
            Some(types) => types.contains(&asked.kind),
            None => asked.kind != "document",
        };
        kind_fits
            && !self.not_types.contains(&asked.kind)
            && (self.url.as_ref()).is_none_or(|url| url.is_match(asked.url).unwrap_or(false))
            && self
                .first_party
                .is_none_or(|first| first != asked.third_party)
            && (self.same_host).is_none_or(|same| same == (asked.host == asked.source_host))
            && self.to.matches(&asked.host)
            && self.from.matches(&asked.source_host)
    }

    /// Whether it's for `method`, or None while it isn't known.
    fn is_for_method(&self, method: Option<&str>) -> Option<bool> {
        if self.methods.is_empty() {
            return Some(true);
        }
        let method = method?.to_ascii_lowercase();
        let named = |negated: bool| {
            (self.methods.iter()).any(|(not, name)| *not == negated && *name == method)
        };
        let has_positive = self.methods.iter().any(|(not, _)| !not);
        Some(!named(true) && (!has_positive || named(false)))
    }

    /// Whether its address test passes for `asked`, false for a header's.
    fn address_matches(&self, asked: &Asked) -> bool {
        match (&self.test, &asked.address) {
            (Test::Address(pattern), Some(address)) => pattern.is_match(address).unwrap_or(false),
            _ => false,
        }
    }
}

/// A request, as the filters read it.
pub struct Asked<'a> {
    pub url: &'a str,
    pub kind: &'a str,
    /// Lowercase, without a port, and an IPv6 address without its brackets.
    pub host: String,
    pub source_host: String,
    /// Its host, if it's written as an IP address.
    pub address: Option<String>,
    pub third_party: bool,
}

/// The host of `url`, lowercase, without its user, port or an IPv6 address's brackets.
pub fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or("", |(_, rest)| rest);
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = match host.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(v6),
        None => host.split(':').next().unwrap_or(host),
    };
    host.to_ascii_lowercase()
}

/// `host` if it's an IP address, as uBlock Origin's getIPAddress gives one: an IPv6 address, which
/// a URL writes in brackets, or four numbers between dots.
pub fn address_of(url: &str, host: &str) -> Option<String> {
    let after = url.split_once("://").map(|(_, rest)| rest)?;
    let after = after.rsplit_once('@').map_or(after, |(_, rest)| rest);
    let v4 = host.split('.').count() == 4
        && host
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
    (after.starts_with('[') || v4).then(|| host.to_string())
}

/// What a request's filters of these say as it starts.
#[derive(Debug, PartialEq, Eq)]
pub enum AtRequest {
    /// Blocked by its host's address, answered by the stand-in named, if any.
    Block(Option<String>),
    /// Asked about again at its responses, with the values of these headers, lowercase.
    Ask(Vec<String>),
}

/// The lists' filters of these, and their lines, which the cache keeps.
#[derive(Default)]
pub struct Responses {
    filters: Vec<Filter>,
    lines: String,
}

impl Responses {
    /// The filters of `lines`, with each it can't honour and why.
    pub fn of(lines: &str) -> (Self, Vec<(String, NotHonoured)>) {
        let mut responses = Self::default();
        let mut not_honoured = Vec::new();
        for line in lines.lines() {
            match Filter::parse(line) {
                Ok(Some(filter)) => {
                    responses.filters.push(filter);
                    responses.lines.push_str(line);
                    responses.lines.push('\n');
                }
                Ok(None) => {}
                Err(why) => not_honoured.push((line.to_string(), why)),
            }
        }
        (responses, not_honoured)
    }

    /// The lines of the filters it has, for `of` again.
    pub fn lines(&self) -> &str {
        &self.lines
    }

    /// What its filters say about `asked` as it starts. `excepted` says whether an exception of
    /// adblock-rust's lets it go, which only an important filter overrides.
    pub fn at_request(&self, asked: &Asked, excepted: impl Fn() -> bool) -> Option<AtRequest> {
        let mut block: Option<&Filter> = None;
        let mut ask: Vec<String> = Vec::new();
        let mut later = false;
        let mut important = false;
        for filter in self.filters.iter().filter(|filter| !filter.exception) {
            if !filter.is_for(asked) {
                continue;
            }
            let method = filter.is_for_method(None);
            match &filter.test {
                // uBlock Origin filters no page's own headers.
                Test::Header { .. } if asked.kind == "document" => {}
                Test::Header { name, .. } => {
                    if !ask.contains(name) {
                        ask.push(name.clone());
                    }
                    important |= filter.important;
                }
                Test::Address(_) if filter.address_matches(asked) => match method {
                    Some(true) if block.is_none_or(|block| !block.important) => {
                        block = Some(filter);
                    }
                    // Its method comes with the response.
                    None => {
                        later = true;
                        important |= filter.important;
                    }
                    _ => {}
                },
                Test::Address(_) => {}
            }
        }
        if block.is_none() && ask.is_empty() && !later {
            return None;
        }
        let excepted = || excepted() || self.excepted(asked, None, "");
        if let Some(block) = block
            && (block.important || !excepted())
        {
            return Some(AtRequest::Block(block.redirect.clone()));
        }
        ((!ask.is_empty() || later) && (important || !excepted())).then_some(AtRequest::Ask(ask))
    }

    /// Whether its filters block `asked` at a response to the request, with `method`, whose
    /// `headers` are lines of "name: value", those its answer as it started named. A redirect's
    /// response is one, before the request goes on to the next address.
    pub fn at_response(
        &self,
        asked: &Asked,
        method: &str,
        headers: &str,
        excepted: impl Fn() -> bool,
    ) -> bool {
        let matches = |filter: &&Filter| {
            filter.is_for(asked)
                && filter.is_for_method(Some(method)) == Some(true)
                && test_passes(filter, asked, headers)
        };
        let blocks: Vec<&Filter> = (self.filters.iter())
            .filter(|filter| !filter.exception)
            .filter(matches)
            .collect();
        if blocks.is_empty() {
            return false;
        }
        blocks.iter().any(|filter| filter.important)
            || !(excepted() || self.excepted(asked, Some(method), headers))
    }

    /// Whether an exception of these lets `asked` go: one of its address's as it starts, and at a
    /// response one of its headers' too.
    fn excepted(&self, asked: &Asked, method: Option<&str>, headers: &str) -> bool {
        (self.filters.iter())
            .filter(|filter| filter.exception)
            .any(|filter| {
                filter.is_for(asked)
                    && filter.is_for_method(method) == Some(true)
                    && match filter.test {
                        Test::Address(_) => filter.address_matches(asked),
                        Test::Header { .. } => {
                            method.is_some() && test_passes(filter, asked, headers)
                        }
                    }
            })
    }

    /// Whether it has none.
    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }
}

/// Whether `filter`'s test passes for `asked` with the response's `headers`.
fn test_passes(filter: &Filter, asked: &Asked, headers: &str) -> bool {
    match &filter.test {
        Test::Address(_) => filter.address_matches(asked),
        Test::Header { name, not, value } => {
            let found = headers.lines().find_map(|line| {
                let (header, value) = line.split_once(':')?;
                header
                    .trim()
                    .eq_ignore_ascii_case(name)
                    .then(|| value.trim())
            });
            match (found, value) {
                (None, _) => *not,
                (Some(_), None) => true,
                (Some(found), Some(value)) => value.is_match(found).unwrap_or(false) != *not,
            }
        }
    }
}

/// `text` apart from the filters of these in it, and those lines.
pub fn apart(text: String) -> (String, String) {
    if !text.contains("header=") && !text.contains("ipaddress=") {
        return (text, String::new());
    }
    let is_one = |line: &str| {
        let cosmetic = ["##", "#@#", "#?#", "#$#", "#@?#", "#@$#", "#%#"]
            .iter()
            .any(|at| line.contains(at));
        !line.starts_with('!')
            && !cosmetic
            && options_at(line).is_some_and(|at| {
                options(&line[at + 1..]).iter().any(|option| {
                    let name = option.split_once('=').map_or(*option, |(name, _)| name);
                    matches!(name, "header" | "ipaddress" | "requestheader")
                })
            })
    };
    let (mut rest, mut lines) = (String::with_capacity(text.len()), String::new());
    for line in text.lines() {
        let to = if is_one(line) { &mut lines } else { &mut rest };
        to.push_str(line);
        to.push('\n');
    }
    (rest, lines)
}

/// The stand-ins' data: URLs by each name and alias, for a `$redirect` of these.
pub fn stand_in_urls(stand_ins: &[adblock::resources::Resource]) -> HashMap<String, String> {
    let mut urls = HashMap::new();
    for stand_in in stand_ins {
        let adblock::resources::ResourceType::Mime(mime) = &stand_in.kind else {
            continue;
        };
        let url = format!("data:{mime};base64,{}", stand_in.content);
        for name in std::iter::once(&stand_in.name).chain(&stand_in.aliases) {
            urls.insert(name.clone(), url.clone());
        }
    }
    urls
}

#[cfg(test)]
mod tests;
