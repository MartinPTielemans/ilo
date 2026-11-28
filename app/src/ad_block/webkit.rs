//! The filter lists as WebKit's content rule lists, so a tab in the system's engine blocks what the
//! same site's tab blocks in ilo's (#3596). The format is Apple's ("Creating a content blocker"
//! and WKContentRuleList's pages). Each filter is read by adblock-rust's parse, as `compile` reads
//! it, so the browser has one filter parser. The parse hashes a filter's sites and keeps no
//! pattern WebKit could read, so the sites and the pattern are read here from the filter's line.
//!
//! What carries over: blocks and exceptions by address, type, party, the page's site and, where
//! WebKit acts on it, method; and hiding by a plain CSS selector. What doesn't is counted in
//! `Counts::left_out` by why. A rule WebKit can't hold exactly is left out, never made broader: a
//! field WebKit doesn't know is dropped and its rule then matches more.
//!
//! Pages and frames aren't in the lists. Their navigations are asked about through ilo's engine,
//! as `PageThrottle` asks for a tab in Chromium, which shows the warning page a list can't, so a
//! rule's `document` and `subdocument` types are left to that. Pop-ups and stand-ins are left out
//! the same way: WebKit can't answer a request with a stand-in, and a pop-up is a new view.
//!
//! The lists are spread as #3596 step 0 measured: about `LIST_RULES` rules a list, so compiling
//! one holds the main thread under 50 ms. A list doesn't see another's exceptions, so each one is
//! in every list.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;

use adblock::filters::cosmetic::{CosmeticFilter, CosmeticFilterMask};
use adblock::filters::network::{NetworkFilter, NetworkFilterMask as Mask};
use adblock::lists::{FilterFormat, FilterParseError, ParseOptions, ParsedLine, parse_filter};
use serde_json::{Map, Value, json};

use super::{
    Setup, hostnames_anchored, if_evaluated, is_hostname, options, popups_apart, responses,
};

/// What the running WebKit acts on, of the fields not every version does, as the platform layer
/// knows from its version. WebKit ignores a field it doesn't know, so its rule would match more.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Features {
    /// `unless-frame-url`, which a filter only for sites but some needs (`domain=~a.com`).
    pub unless_frame_url: bool,
    /// `request-method`, which macOS 15's WebKit ignores and 26's acts on.
    pub request_method: bool,
}

/// About how many rules a list holds, so compiling it holds the main thread under 50 ms on a hosted
/// runner (#3596 step 0, spread16).
pub const LIST_RULES: usize = 14_000;

/// The most rules WebKit compiles in one list; one more and the list fails.
pub const WEBKIT_LIMIT: usize = 150_000;

/// Selectors in one hiding rule.
const SELECTORS: usize = 100;

/// One content rule list, as WKContentRuleListStore compiles it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct List {
    /// Its identifier in the store, from its rules, so a list that didn't change isn't compiled
    /// again.
    pub name: String,
    /// The rules, a JSON array.
    pub json: String,
    pub rules: usize,
    /// Of `rules`, the copies of every list's exceptions.
    pub exceptions: usize,
    /// Whether it's the user's own filters, whose list changes as they write.
    pub user: bool,
}

/// The lists, and what of the filters they hold.
#[derive(Debug, Default)]
pub struct Lists {
    pub lists: Vec<List>,
    pub counts: Counts,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Filters read, the lists' and the user's.
    pub filters: usize,
    /// Rules in all the lists, each copy of an exception counted.
    pub rules: usize,
    /// Filter lists that couldn't be read.
    pub unread: usize,
    /// Filters, or rules for the limit, that aren't in the lists, by why.
    pub left_out: BTreeMap<&'static str, usize>,
}

impl Counts {
    fn leave_out(&mut self, why: &'static str) {
        *self.left_out.entry(why).or_default() += 1;
    }
}

/// Where a rule goes in its list. WebKit applies a list's rules in order, and an exception
/// (ignore-previous-rules) undoes what the rules before it did, so `$important` blocks come after.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Place {
    Hide,
    Block,
    Exception,
    Important,
    ImportantException,
}

impl Place {
    /// Whether every list has a copy.
    fn shared(self) -> bool {
        matches!(self, Place::Exception | Place::ImportantException)
    }
}

/// A rule, as JSON, and whose it is.
struct Rule {
    place: Place,
    json: String,
    user: bool,
}

/// The start of an address up to its host, any subdomain of it included, as `||` means.
const HOST_START: &str = r"^[^:]+://+([^:/]+\.)?";

/// One character `^` stands for: one that can't be in a host or a word.
const SEPARATOR: &str = r"[^a-z0-9_.%-]";

/// WebKit's types for adblock-rust's, of a request a page makes. Pages and frames are the
/// navigation check's.
const TYPES: &[(Mask, &[&str])] = &[
    (Mask::FROM_IMAGE, &["image", "svg-document"]),
    (Mask::FROM_MEDIA, &["media"]),
    (Mask::FROM_OBJECT, &["other"]),
    (Mask::FROM_OTHER, &["other", "raw"]),
    (Mask::FROM_PING, &["ping"]),
    (Mask::FROM_SCRIPT, &["script"]),
    (Mask::FROM_STYLESHEET, &["style-sheet"]),
    (Mask::FROM_WEBSOCKET, &["websocket"]),
    (Mask::FROM_XMLHTTPREQUEST, &["fetch"]),
    (Mask::FROM_FONT, &["font"]),
];

/// `setup`'s lists and the user's filters as WebKit's lists. It reads them as `compile` does, and
/// takes about as long, off the main thread.
pub fn convert(setup: &Setup, features: Features) -> Lists {
    let mut sources = Vec::with_capacity(setup.lists.len() + 1);
    let mut unread = 0;
    for list in &setup.lists {
        match fs::read_to_string(list) {
            Ok(text) => {
                let text = if_evaluated(text);
                let options = options(&setup.folder, list, &text);
                sources.push(Source {
                    text,
                    options,
                    user: false,
                });
            }
            Err(_) => unread += 1,
        }
    }
    sources.push(Source {
        text: if_evaluated(setup.filters.clone()),
        options: ParseOptions::default(),
        user: true,
    });
    let mut lists = from_sources(sources, features);
    lists.counts.unread = unread;
    lists
}

/// A list's text, or the user's filters, and how to read it.
struct Source {
    text: String,
    options: ParseOptions,
    user: bool,
}

/// One filter as the parse read it, and its line.
struct Read<'a> {
    line: &'a str,
    parsed: ParsedLine<'a>,
    hosts_format: bool,
    user: bool,
}

fn from_sources(sources: Vec<Source>, features: Features) -> Lists {
    let mut counts = Counts::default();
    // Taken apart as `compile` does: what adblock-rust doesn't read is left out here too.
    let texts: Vec<(String, &Source)> = sources
        .iter()
        .map(|source| {
            let text = match source.options.format {
                FilterFormat::Standard => {
                    let (text, responses) = responses::apart(source.text.clone());
                    for _ in responses.lines().filter(|line| !line.trim().is_empty()) {
                        counts.leave_out("a response-time filter (header=, ipaddress=)");
                    }
                    let (text, popups) = popups_apart(text);
                    for _ in popups.lines().filter(|line| !line.trim().is_empty()) {
                        counts.leave_out("a pop-up filter: the navigation check's");
                    }
                    text
                }
                FilterFormat::Hosts => source.text.clone(),
            };
            (hostnames_anchored(text), source)
        })
        .collect();
    let mut read = Vec::new();
    for (text, source) in &texts {
        let hosts_format = matches!(source.options.format, FilterFormat::Hosts);
        for line in text.lines().map(str::trim) {
            if line.is_empty()
                || line.starts_with(['!', '['])
                || (hosts_format && line.starts_with('#'))
            {
                continue;
            }
            counts.filters += 1;
            match parse_filter(line, false, source.options) {
                Ok(parsed) => read.push(Read {
                    line,
                    parsed,
                    hosts_format,
                    user: source.user,
                }),
                Err(error) => counts.leave_out(match error {
                    FilterParseError::Network(_) => "a network filter adblock-rust refuses",
                    FilterParseError::Cosmetic(_) => "a hiding filter adblock-rust refuses",
                    _ => "a line adblock-rust doesn't read",
                }),
            }
        }
    }
    // `$badfilter` takes the same filter without it out of every list, as FilterSet does.
    let bad: HashSet<_> = read
        .iter()
        .filter_map(|read| match &read.parsed {
            ParsedLine::Network(filter) if filter.is_badfilter() => Some(filter.get_id()),
            _ => None,
        })
        .collect();
    let mut rules = Vec::new();
    let mut hiding = Hiding::default();
    for read in &read {
        let result = match &read.parsed {
            ParsedLine::Network(filter) if filter.is_badfilter() => continue,
            ParsedLine::Network(filter) if bad.contains(&filter.get_id()) => continue,
            ParsedLine::Network(filter) => network(filter, read, features, &mut rules, &mut hiding),
            ParsedLine::Cosmetic(filter) => hiding.add(filter, read),
        };
        if let Err(why) = result {
            counts.leave_out(why);
        }
    }
    hiding.rules(&mut rules, &mut counts);
    let lists = spread(rules, &mut counts);
    counts.rules = lists.iter().map(|list| list.rules).sum();
    Lists { lists, counts }
}

/// The filter's pattern: its line without `@@` and its options.
fn pattern_of(line: &str) -> &str {
    let line = line.strip_prefix("@@").unwrap_or(line);
    match responses::options_at(line) {
        Some(at) => &line[..at],
        None => line,
    }
}

/// The filter's options, as uBlock Origin finds them.
fn options_of(line: &str) -> Vec<&str> {
    let line = line.strip_prefix("@@").unwrap_or(line);
    match responses::options_at(line) {
        Some(at) => responses::options(&line[at + 1..]),
        None => Vec::new(),
    }
}

/// The sites one of the filter's options names, `domain=a.com|~b.com`, as (in, out).
fn sites_in(line: &str, names: &[&str]) -> Result<(Vec<String>, Vec<String>), &'static str> {
    let (mut yes, mut no) = (Vec::new(), Vec::new());
    for option in options_of(line) {
        let Some((name, value)) = option.split_once('=') else {
            continue;
        };
        if !names.contains(&name) {
            continue;
        }
        let domains =
            responses::Domains::parse(value).map_err(|_| "a site.* or a pattern for a site")?;
        yes.extend(domains.yes);
        no.extend(domains.no);
    }
    yes.sort_unstable();
    yes.dedup();
    no.sort_unstable();
    no.dedup();
    if yes
        .iter()
        .chain(&no)
        .any(|site| !site.is_ascii() || site.contains(':'))
    {
        return Err("a site WebKit can't name (not ASCII, or an IPv6 address)");
    }
    Ok((yes, no))
}

/// Addresses on `site` or a subdomain of it, as a url-filter.
fn on_site(site: &str) -> String {
    format!("{HOST_START}{}[:/]", escaped(site))
}

/// `text` with each character a url-filter would read as more than itself escaped.
fn escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        if r".+?$()[]{}\/|^*".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The url-filters a filter's pattern is, as WebKit's regular expressions: one, or two where a
/// separator `^` ends it, which can be the address's end, as WebKit has no `|`.
fn url_filters(pattern: &str) -> Result<Vec<String>, &'static str> {
    if !pattern.is_ascii() {
        return Err("a non-ASCII address");
    }
    if pattern.len() > 2 && pattern.starts_with('/') && pattern.ends_with('/') {
        let regex = &pattern[1..pattern.len() - 1];
        return match webkit_reads(regex) {
            true => Ok(vec![regex.to_string()]),
            false => Err("a regular expression WebKit doesn't read"),
        };
    }
    let (start, rest) = match (pattern.strip_prefix("||"), pattern.strip_prefix('|')) {
        (Some(rest), _) => (HOST_START, rest),
        (None, Some(rest)) => ("^", rest),
        (None, None) => ("", pattern),
    };
    let (rest, end) = match rest.strip_suffix('|') {
        Some(rest) => (rest, "$"),
        None => (rest, ""),
    };
    let (rest, separator) = match rest.strip_suffix('^') {
        Some(rest) => (rest, true),
        None => (rest, false),
    };
    let mut url = String::from(start);
    for c in rest.chars() {
        match c {
            '*' => url.push_str(".*"),
            '^' => url.push_str(SEPARATOR),
            c => url.push_str(&escaped(&c.to_string())),
        }
    }
    if !separator {
        if url.is_empty() {
            url.push_str(".*");
        }
        url.push_str(end);
        return Ok(vec![url]);
    }
    // A host and nothing after: the next character ends the host.
    if start == HOST_START && is_hostname(rest) {
        url.push_str("[:/?]");
        return Ok(vec![url]);
    }
    Ok(vec![format!("{url}{SEPARATOR}{end}"), format!("{url}$")])
}

/// Whether WebKit's url-filter reads `regex` as a filter's author meant it. It has no `|`, no
/// counted repeats, no back-references, groups that aren't plain, class escapes or lazy repeats,
/// and only ASCII; and a list with one regular expression it can't read fails whole.
fn webkit_reads(regex: &str) -> bool {
    if !regex.is_ascii() || regex.is_empty() {
        return false;
    }
    let bytes = regex.as_bytes();
    let mut class = false;
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        match byte {
            b'\\' => {
                let Some(next) = bytes.get(at + 1) else {
                    return false;
                };
                if next.is_ascii_alphanumeric() {
                    return false;
                }
                at += 2;
                continue;
            }
            b'[' if !class => class = true,
            b']' if class => class = false,
            _ if class => {}
            b'|' | b'{' | b'}' => return false,
            b'(' if bytes.get(at + 1) == Some(&b'?') => return false,
            b'?' | b'*' | b'+' if at > 0 && matches!(bytes[at - 1], b'?' | b'*' | b'+') => {
                return false;
            }
            b'^' if at != 0 => return false,
            b'$' if at != bytes.len() - 1 => return false,
            _ => {}
        }
        at += 1;
    }
    !class
}

/// The rules of a network filter, or why it has none.
fn network(
    filter: &NetworkFilter,
    read: &Read,
    features: Features,
    rules: &mut Vec<Rule>,
    hiding: &mut Hiding,
) -> Result<(), &'static str> {
    let mask = filter.mask;
    let exception = mask.contains(Mask::IS_EXCEPTION);
    if filter.is_redirect() {
        return Err("a stand-in (redirect=): WebKit can't answer with one");
    }
    if filter.is_csp() {
        return Err("csp=");
    }
    if filter.is_removeparam() {
        return Err("removeparam=");
    }
    // A tagged filter is off until its tag is turned on, which ilo's engine never does. uBlock
    // Origin has no `tag`, so its options are after the last `$`.
    let tagged = |line: &str| {
        (line.rfind('$')).is_some_and(|at| line[at + 1..].split(',').any(|o| o.starts_with("tag=")))
    };
    if tagged(read.line) {
        return Err("a tag= filter, off in ilo's engine");
    }
    let pattern = match read.hosts_format {
        true => {
            let host = filter
                .hostname
                .as_deref()
                .ok_or("a hosts line without a host")?;
            format!("||{host}^")
        }
        false => pattern_of(read.line).to_string(),
    };
    // The site the pattern is, for an exception that turns a page's filtering off.
    let site = (pattern.strip_prefix("||"))
        .map(|rest| rest.trim_end_matches('|').trim_end_matches('^'))
        .filter(|host| is_hostname(host))
        .map(str::to_ascii_lowercase);
    let (from, not_from) = sites_in(read.line, &["domain", "from"])?;
    let (to, not_to) = sites_in(read.line, &["to"])?;
    if from.is_empty()
        && not_from.is_empty()
        && (filter.opt_domains.is_some() || filter.opt_not_domains.is_some())
    {
        return Err("a site it didn't read");
    }
    if filter.is_generic_hide() {
        return match (exception, site, from.is_empty() && not_from.is_empty()) {
            (true, Some(site), true) => {
                hiding.generic_off.insert(site);
                Ok(())
            }
            _ => Err("generichide on more than a site"),
        };
    }
    let party = match (
        mask.contains(Mask::THIRD_PARTY),
        mask.contains(Mask::FIRST_PARTY),
    ) {
        (true, true) => None,
        (true, false) => Some("third-party"),
        (false, true) => Some("first-party"),
        (false, false) => return Err("neither party (1p,3p)"),
    };
    let methods = mask & Mask::FROM_ANY_METHODS;
    let method = match methods {
        _ if methods.is_empty() => None,
        _ if !features.request_method => {
            return Err("method= on WebKit that ignores request-method");
        }
        _ if methods == Mask::FROM_GET => Some("get"),
        _ if methods == Mask::FROM_HEAD => Some("head"),
        _ if methods == Mask::FROM_POST => Some("post"),
        _ => return Err("more than one method= (request-method takes one)"),
    };
    let condition: Option<(&str, Vec<String>)> = match (from.is_empty(), not_from.is_empty()) {
        (true, true) => None,
        (false, true) => Some((
            "if-frame-url",
            from.iter().map(|site| on_site(site)).collect(),
        )),
        (true, false) if features.unless_frame_url => Some((
            "unless-frame-url",
            not_from.iter().map(|site| on_site(site)).collect(),
        )),
        (true, false) => return Err("domain=~ on WebKit without unless-frame-url"),
        (false, false) => return Err("domain= with sites in and out (a rule has one condition)"),
    };
    let place = match (exception, mask.contains(Mask::IS_IMPORTANT)) {
        (false, false) => Place::Block,
        (false, true) => Place::Important,
        (true, false) => Place::Exception,
        (true, true) => Place::ImportantException,
    };
    let action = || match exception {
        true => json!({"type": "ignore-previous-rules"}),
        false => json!({"type": "block"}),
    };
    let mut add = |trigger: Map<String, Value>| {
        rules.push(Rule {
            place,
            json: json!({"trigger": trigger, "action": action()}).to_string(),
            user: read.user,
        });
    };
    // `@@||site^$document`: nothing on that site's pages is blocked or hidden.
    if exception && mask.contains(Mask::FROM_DOCUMENT) {
        let Some(site) = site
            .as_ref()
            .filter(|_| condition.is_none() && to.is_empty())
        else {
            return Err("a page exception for more than a site");
        };
        let mut trigger = Map::new();
        trigger.insert("url-filter".into(), ".*".into());
        trigger.insert("if-domain".into(), json!([format!("*{site}")]));
        add(trigger);
    }
    let mut types: Vec<&str> = (TYPES.iter())
        .filter(|(bit, _)| mask.intersects(*bit))
        .flat_map(|(_, names)| names.iter().copied())
        .collect();
    types.sort_unstable();
    types.dedup();
    if types.is_empty() {
        return match mask.intersects(Mask::FROM_DOCUMENT | Mask::FROM_SUBDOCUMENT) {
            true if exception && mask.contains(Mask::FROM_DOCUMENT) => Ok(()),
            true => Err("only pages or frames: the navigation check's"),
            false => Err("no type WebKit has"),
        };
    }
    let urls = match (to.is_empty(), not_to.is_empty()) {
        (true, true) => url_filters(&pattern)?,
        (false, true) if matches!(pattern.as_str(), "" | "*") => {
            if to.iter().any(|site| !site.is_ascii()) {
                return Err("a non-ASCII address");
            }
            to.iter().map(|site| on_site(site)).collect()
        }
        _ => return Err("to= with an address or a site out"),
    };
    for url in urls {
        let mut trigger = Map::new();
        trigger.insert("url-filter".into(), url.into());
        if mask.contains(Mask::MATCH_CASE) {
            trigger.insert("url-filter-is-case-sensitive".into(), true.into());
        }
        trigger.insert("resource-type".into(), json!(types));
        if let Some(party) = party {
            trigger.insert("load-type".into(), json!([party]));
        }
        if let Some((name, sites)) = &condition {
            trigger.insert((*name).into(), json!(sites));
        }
        if let Some(method) = method {
            trigger.insert("request-method".into(), method.into());
        }
        add(trigger);
    }
    Ok(())
}

/// The hiding filters, gathered so their selectors share rules, and their exceptions.
#[derive(Default)]
struct Hiding {
    /// Selectors for every site but some: by whose they are and those sites.
    generic: BTreeMap<(bool, Vec<String>), Vec<String>>,
    /// Selectors for some sites only: by whose they are and those sites.
    specific: BTreeMap<(bool, Vec<String>), Vec<String>>,
    /// Selectors `site#@#` turns off on some sites, and those sites.
    off_at: BTreeMap<String, BTreeSet<String>>,
    /// Sites `$generichide` turns every site's selectors off on.
    generic_off: BTreeSet<String>,
}

/// Whether `site` is `under` or a subdomain of it.
fn is_under(site: &str, under: &str) -> bool {
    site == under || (site.strip_suffix(under)).is_some_and(|start| start.ends_with('.'))
}

impl Hiding {
    fn add(&mut self, filter: &CosmeticFilter, read: &Read) -> Result<(), &'static str> {
        if filter.mask.contains(CosmeticFilterMask::SCRIPT_INJECT) {
            return Err("a scriptlet (+js)");
        }
        if filter.action.is_some() {
            return Err("a style or remove action");
        }
        let Some(selector) = filter.plain_css_selector() else {
            return Err("a procedural selector");
        };
        if !selector.is_ascii() || selector.contains(['\n', '\r']) {
            return Err("a non-ASCII selector");
        }
        if filter.entities.is_some() || filter.not_entities.is_some() {
            return Err("a site.* for hiding");
        }
        // Sites come before the first `#`, which no site has.
        let before = read.line.find('#').map_or("", |at| &read.line[..at]);
        let (mut yes, mut no) = (Vec::new(), Vec::new());
        for site in before
            .split(',')
            .map(str::trim)
            .filter(|site| !site.is_empty())
        {
            let (not, site) = match site.strip_prefix('~') {
                Some(site) => (true, site),
                None => (false, site),
            };
            if !site.is_ascii() || site.starts_with('/') || site.contains(':') {
                return Err("a site WebKit can't name (not ASCII, or a pattern)");
            }
            match not {
                true => no.push(site.to_ascii_lowercase()),
                false => yes.push(site.to_ascii_lowercase()),
            }
        }
        if yes.is_empty() != filter.hostnames.is_none()
            || no.is_empty() != filter.not_hostnames.is_none()
        {
            return Err("a site it didn't read");
        }
        yes.sort_unstable();
        yes.dedup();
        no.sort_unstable();
        no.dedup();
        let selector = selector.to_string();
        if filter.mask.contains(CosmeticFilterMask::UNHIDE) {
            // adblock-rust refuses `#@#` without a site, as uBlock Origin does.
            match no.is_empty() {
                true => self.off_at.entry(selector).or_default().extend(yes),
                false => return Err("#@# with a site out"),
            }
            return Ok(());
        }
        match (yes.is_empty(), no.is_empty()) {
            (true, _) => self
                .generic
                .entry((read.user, no))
                .or_default()
                .push(selector),
            (false, true) => self
                .specific
                .entry((read.user, yes))
                .or_default()
                .push(selector),
            (false, false) => {
                return Err("hiding with sites in and out (a rule has one condition)");
            }
        }
        Ok(())
    }

    /// The hiding rules, with what turns them off.
    fn rules(self, rules: &mut Vec<Rule>, counts: &mut Counts) {
        let mut add = |user: bool, condition: Option<(&str, Vec<String>)>, selectors: &[&str]| {
            for batch in selectors.chunks(SELECTORS) {
                let mut trigger = Map::new();
                trigger.insert("url-filter".into(), ".*".into());
                if let Some((name, sites)) = &condition {
                    let sites: Vec<String> = sites.iter().map(|site| format!("*{site}")).collect();
                    trigger.insert((*name).into(), json!(sites));
                }
                let action = json!({"type": "css-display-none", "selector": batch.join(", ")});
                rules.push(Rule {
                    place: Place::Hide,
                    json: json!({"trigger": trigger, "action": action}).to_string(),
                    user,
                });
            }
        };
        let unless = |sites: Vec<String>| (!sites.is_empty()).then_some(("unless-domain", sites));
        for ((user, out), selectors) in &self.generic {
            let out: BTreeSet<String> = out.iter().chain(&self.generic_off).cloned().collect();
            let mut plain = Vec::new();
            for selector in selectors {
                match self.off_at.get(selector) {
                    None => plain.push(selector.as_str()),
                    Some(off) => {
                        let sites = out.iter().chain(off).cloned().collect::<BTreeSet<_>>();
                        add(
                            *user,
                            unless(sites.into_iter().collect()),
                            &[selector.as_str()],
                        );
                    }
                }
            }
            add(*user, unless(out.into_iter().collect()), &plain);
        }
        for ((user, sites), selectors) in &self.specific {
            let mut by_sites: BTreeMap<Vec<String>, Vec<&str>> = BTreeMap::new();
            for selector in selectors {
                let off = self.off_at.get(selector);
                let off_on =
                    |site: &str| off.is_some_and(|off| off.iter().any(|o| is_under(site, o)));
                let kept: Vec<String> =
                    sites.iter().filter(|site| !off_on(site)).cloned().collect();
                // Off on a subdomain of a site it's for: if-domain can't say all but it.
                let part = off.is_some_and(|off| {
                    off.iter()
                        .any(|o| kept.iter().any(|site| is_under(o, site) && o != site))
                });
                if part {
                    counts.leave_out("hiding turned off on part of its site");
                } else if !kept.is_empty() {
                    by_sites.entry(kept).or_default().push(selector);
                }
            }
            for (sites, selectors) in by_sites {
                add(*user, Some(("if-domain", sites)), &selectors);
            }
        }
    }
}

/// FNV-1a, so a rule's list and a list's name stay the same from one run to the next.
fn fnv(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// The rules spread over lists of about `LIST_RULES`, each with a copy of every exception, and the
/// user's own in a list of their own so writing one compiles only that list and, for an
/// exception, every list.
fn spread(rules: Vec<Rule>, counts: &mut Counts) -> Vec<List> {
    let (shared, own): (Vec<Rule>, Vec<Rule>) =
        rules.into_iter().partition(|rule| rule.place.shared());
    let (user, lists): (Vec<Rule>, Vec<Rule>) = own.into_iter().partition(|rule| rule.user);
    let room = LIST_RULES.saturating_sub(shared.len()).max(LIST_RULES / 4);
    let count = lists
        .len()
        .div_ceil(room)
        .max(usize::from(!shared.is_empty()));
    let mut buckets: Vec<Vec<Rule>> = (0..count).map(|_| Vec::new()).collect();
    for rule in lists {
        let at = (fnv(&rule.json) % count as u64) as usize;
        buckets[at].push(rule);
    }
    let mut out: Vec<(Vec<Rule>, bool)> =
        buckets.into_iter().map(|bucket| (bucket, false)).collect();
    if !user.is_empty() {
        out.push((user, true));
    }
    let shared_room = WEBKIT_LIMIT - 1;
    let shared = match shared.len() > shared_room {
        true => {
            for _ in shared_room..shared.len() {
                counts.leave_out("an exception past WebKit's limit");
            }
            &shared[..shared_room]
        }
        false => &shared[..],
    };
    out.into_iter()
        .map(|(mut own, user)| {
            own.sort_by_key(|rule| rule.place);
            let room = WEBKIT_LIMIT - shared.len();
            if own.len() > room {
                for _ in room..own.len() {
                    counts.leave_out("a rule past WebKit's limit");
                }
                own.truncate(room);
            }
            let mut ordered: Vec<&Rule> = own.iter().chain(shared).collect();
            ordered.sort_by_key(|rule| rule.place);
            let mut json = String::with_capacity(
                ordered
                    .iter()
                    .map(|rule| rule.json.len() + 1)
                    .sum::<usize>()
                    + 2,
            );
            json.push('[');
            for (at, rule) in ordered.iter().enumerate() {
                if at > 0 {
                    json.push(',');
                }
                json.push_str(&rule.json);
            }
            json.push(']');
            List {
                name: format!("ilo-{:016x}", fnv(&json)),
                json,
                rules: ordered.len(),
                exceptions: shared.len(),
                user,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
