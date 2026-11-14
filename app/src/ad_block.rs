//! The browser's own ad blocker (#2111): Brave's adblock-rust on the lists uBlock Origin comes with,
//! in place of uBlock Origin with `--ad-block-engine=native`. The core finds the requests pages make
//! and asks the app about each (core/native_ad_block.h), which `ad_block_host.rs` answers from the
//! engine here, on the core's sequence, never the main thread.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use adblock::Engine as Rules;
use adblock::lists::{FilterFormat, FilterSet, ParseOptions};
use adblock::request::Request;
use adblock::resources::{MimeType, PermissionMask, Resource, ResourceType};
use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;
use web_time::Instant;

pub mod choice;
mod responses;
mod scriptlets;

/// What the engine blocks with, as the core gives it (bc_host's ad_block_use).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Setup {
    /// uBlock Origin's folder as the browser comes with it, whose stand-ins filters name.
    pub folder: PathBuf,
    /// The files of the filter lists turned on.
    pub lists: Vec<PathBuf>,
    /// The user's own filters, one a line.
    pub filters: String,
}

/// The compiled lists: what blocks, redirects and rewrites requests and what pages hide and run,
/// and apart, the filters for pop-ups, as uBlock Origin keeps them. adblock-rust drops every `$popup`
/// filter, and would match one without a type against a pop-up, which uBlock Origin never does.
pub struct Engine {
    rules: Rules,
    popups: Rules,
    /// uBlock Origin's response-time filters, which adblock-rust doesn't read.
    responses: responses::Responses,
    /// The stand-ins' data: URLs by name, for those filters' `$redirect`.
    stand_ins: std::collections::HashMap<String, String>,
}

/// What to do with a request (BC_AD_BLOCK_).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Block,
    /// Answered in its place with this data: URL.
    Redirect(String),
    /// Sent to this address instead.
    Rewrite(String),
    /// Allowed now, and asked about again at each of its responses, a redirect's too, with the
    /// values of these headers: `at_response` says then.
    AtResponse(Vec<String>),
}

/// The bit of adblock-rust's permission mask uBlock Origin's own lists carry, which its trusted
/// scriptlets require, as its `requiresTrust` does.
const TRUSTED: u8 = 1;

/// How to read a list: uBlock Origin's own may run its trusted scriptlets, and one mostly of a hosts
/// file's lines, as Peter Lowe's is, is read as one, as uBlock Origin reads it.
fn options(folder: &Path, list: &Path, text: &str) -> ParseOptions {
    let (mut hosts, mut all) = (0, 0);
    for line in text
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with(['!', '#']))
    {
        all += 1;
        if line.starts_with("127.0.0.1 ") || line.starts_with("0.0.0.0 ") {
            hosts += 1;
        }
    }
    let own = list.starts_with(folder.join("assets").join("ublock"));
    ParseOptions {
        format: match hosts * 2 > all {
            true => FilterFormat::Hosts,
            false => FilterFormat::Standard,
        },
        permissions: PermissionMask::from_bits(if own { TRUSTED } else { 0 }),
        ..Default::default()
    }
}

/// Whether `pattern` is a hostname as uBlock Origin's filter parser takes one, its reHostnameAscii,
/// in any case: labels of letters, digits, `-` and `_` that start with a letter or digit, the last
/// with no `_`, ending in a letter or digit and at least two long.
fn is_hostname(pattern: &str) -> bool {
    let edge = |byte: &u8| byte.is_ascii_alphanumeric();
    let mut labels = pattern.split('.').map(str::as_bytes).rev();
    let Some(last) = labels.next() else {
        return false;
    };
    last.len() >= 2
        && edge(&last[0])
        && edge(&last[last.len() - 1])
        && last
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
        && labels.all(|label| {
            label.first().is_some_and(edge)
                && label
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        })
}

/// `text` with each line that is only a hostname, as URLhaus's are, read as uBlock Origin reads it:
/// that host and its subdomains, `||host^`, which blocks a page there too. adblock-rust reads one as
/// part of any address, so `1.2.3.44` matched `11.2.3.44` and never the page at `1.2.3.44`.
fn hostnames_anchored(text: String) -> String {
    let anchored = |line: &str| {
        let (exception, pattern) = match line.strip_prefix("@@") {
            Some(pattern) => ("@@", pattern),
            None => ("", line),
        };
        is_hostname(pattern).then(|| format!("{exception}||{}^", pattern.to_ascii_lowercase()))
    };
    if !text.lines().any(|line| anchored(line).is_some()) {
        return text;
    }
    let mut out = String::with_capacity(text.len() + text.len() / 8);
    for line in text.lines() {
        match anchored(line) {
            Some(line) => out.push_str(&line),
            None => out.push_str(line),
        }
        out.push('\n');
    }
    out
}

/// What a list's `!#if` tokens stand for, as uBlock Origin 1.75's preparser maps them.
const IF_TOKENS: &[(&str, &str)] = &[
    ("ext_ublock", "ublock"),
    ("ext_ubol", "ubol"),
    ("ext_devbuild", "devbuild"),
    ("env_brave", "brave"),
    ("env_chromium", "chromium"),
    ("env_edge", "edge"),
    ("env_firefox", "firefox"),
    ("env_legacy", "legacy"),
    ("env_mobile", "mobile"),
    ("env_mv3", "mv3"),
    ("env_safari", "safari"),
    ("cap_html_filtering", "html_filtering"),
    ("cap_ipaddress", "ipaddress"),
    ("false", "false"),
    ("ext_abp", "false"),
    ("adguard", "adguard"),
    ("adguard_app_android", "false"),
    ("adguard_app_cli", "false"),
    ("adguard_app_ios", "false"),
    ("adguard_app_mac", "false"),
    ("adguard_app_windows", "false"),
    ("adguard_ext_android_cb", "false"),
    ("adguard_ext_chromium", "chromium"),
    ("adguard_ext_chromium_mv3", "mv3"),
    ("adguard_ext_edge", "edge"),
    ("adguard_ext_firefox", "firefox"),
    ("adguard_ext_opera", "chromium"),
    ("adguard_ext_safari", "false"),
];

/// What holds for uBlock Origin on Chromium, its webextFlavor's soup there.
const IF_HOLDS: &[&str] = &["ublock", "webext", "ipaddress", "chromium"];

/// Whether `token` of an `!#if` holds, or None for one that's unknown, as the preparser's
/// evaluateExprToken: an unknown `cap_` token is false.
fn if_token(token: &str) -> Option<bool> {
    let (not, token) = match token.strip_prefix('!') {
        Some(token) => (true, token),
        None => (false, token),
    };
    let state = match IF_TOKENS.iter().find(|(name, _)| *name == token) {
        Some((_, state)) => *state,
        None if token.starts_with("cap_") => "false",
        None => return None,
    };
    Some(state == "false" && not || IF_HOLDS.contains(&state) != not)
}

/// Whether the condition `expression` of an `!#if` holds, or None where it's unknown, as the
/// preparser's evaluateExpr: tokens joined by `&&` and `||` read left to right, in one pair of
/// brackets or none. An unknown first token stays unknown through `&&` and gives way to `||`, as
/// in JavaScript; any other unknown token makes it all unknown.
fn if_holds(expression: &str) -> Option<bool> {
    let expression = expression
        .strip_prefix('(')
        .and_then(|inner| inner.strip_suffix(')'))
        .unwrap_or(expression);
    let tokens: Vec<&str> = expression.split_whitespace().collect();
    let first = tokens.first()?;
    if first.starts_with(['|', '&']) {
        return None;
    }
    let mut result = if_token(first);
    for pair in tokens[1..].chunks(2) {
        let [operator, operand] = pair else {
            return None;
        };
        let state = if_token(operand)?;
        result = match *operator {
            "||" if result == Some(true) => result,
            "||" => Some(state),
            "&&" if result == Some(true) => Some(state),
            "&&" => result,
            _ => return None,
        };
    }
    result
}

/// Which of `!#if`, `!#else` and `!#endif` `line` is, if any, and what follows the word.
fn if_directive(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("!#")?;
    ["if", "else", "endif"].into_iter().find_map(|word| {
        let after = rest.strip_prefix(word)?;
        let ends = !after.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_');
        ends.then_some((word, after))
    })
}

/// `text` without the lines its `!#if` conditions leave out for uBlock Origin on Chromium, as its
/// preparser prunes every list before compiling it. adblock-rust reads the directives as comments,
/// so it would compile every branch, Firefox's, Safari's and uBO Lite's too (#2760). A condition
/// that's unknown keeps its lines.
fn if_evaluated(text: String) -> String {
    if !text.contains("!#if") {
        return text;
    }
    // Each open `!#if`: whether its condition is known, and whether the branch it's in goes.
    let mut open: Vec<(bool, bool)> = Vec::new();
    let mut discard = false;
    let goes = |open: &[(bool, bool)]| open.iter().any(|&(known, goes)| known && goes);
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        match if_directive(line) {
            None if !discard => {
                out.push_str(line);
                out.push('\n');
            }
            None => {}
            Some(("if", condition)) => {
                let holds = if_holds(condition.trim());
                open.push((holds.is_some(), holds == Some(false)));
                discard = goes(&open);
            }
            Some(("else", _)) => {
                if let Some((_, branch_goes)) = open.last_mut() {
                    *branch_goes = !*branch_goes;
                    discard = goes(&open);
                }
            }
            Some(_) => {
                open.pop();
                discard = goes(&open);
            }
        }
    }
    out
}

/// The request types a filter's options can name, as uBlock Origin and adblock-rust write them.
const TYPES: &[&str] = &[
    "doc",
    "document",
    "frame",
    "subdocument",
    "script",
    "image",
    "xhr",
    "xmlhttprequest",
    "css",
    "stylesheet",
    "media",
    "font",
    "object",
    "ping",
    "websocket",
    "other",
];

/// The pattern and options of `line` if it's a network filter for pop-ups.
fn popup_filter(line: &str) -> Option<(&str, Vec<&str>)> {
    let cosmetic = ["##", "#@#", "#?#", "#$#", "#@?#", "#@$#"]
        .iter()
        .any(|at| line.contains(at));
    let at = line
        .rfind('$')
        .filter(|_| !line.starts_with('!') && !cosmetic)?;
    let options: Vec<&str> = line[at + 1..].split(',').collect();
    options.contains(&"popup").then(|| (&line[..at], options))
}

/// `text` apart from its `$popup` filters, and those filters, each in the popups' engine without
/// its types, since all it's asked about is pop-ups. A filter for pop-ups and other types too, as
/// `$doc,popup` is, keeps the others in `text`: adblock-rust drops all of one that names `popup`.
fn popups_apart(text: String) -> (String, String) {
    if !text.lines().any(|line| popup_filter(line).is_some()) {
        return (text, String::new());
    }
    let (mut rest, mut popups) = (String::with_capacity(text.len()), String::new());
    for line in text.lines() {
        let Some((pattern, options)) = popup_filter(line) else {
            rest.push_str(line);
            rest.push('\n');
            continue;
        };
        let is_type = |option: &&str| TYPES.contains(&option.trim_start_matches('~'));
        let (types, others): (Vec<&str>, Vec<&str>) = options
            .iter()
            .copied()
            .filter(|option| *option != "popup")
            .partition(is_type);
        // An exception's pattern keeps its @@, and one of only options, any address.
        let pattern = match pattern.trim_start_matches("@@") {
            "" => format!("{pattern}*"),
            _ => pattern.to_string(),
        };
        popups.push_str(&pattern);
        if !others.is_empty() {
            popups.push('$');
            popups.push_str(&others.join(","));
        }
        popups.push('\n');
        if types.iter().any(|option| !option.starts_with('~')) {
            let kept: Vec<&str> = options
                .into_iter()
                .filter(|option| *option != "popup")
                .collect();
            rest.push_str(&format!("{pattern}${}\n", kept.join(",")));
        }
    }
    (rest, popups)
}

/// Compiles `setup`'s lists and the user's filters, about 0.25 s for the lists uBlock Origin turns
/// on, and gives it uBlock Origin's stand-ins and scriptlets. A list that can't be read is left out.
pub fn compile(setup: &Setup) -> Engine {
    let (mut set, mut popups) = (FilterSet::new(false), FilterSet::new(false));
    let mut response_lines = String::new();
    let mut add = |text: String, options: ParseOptions| {
        let (text, popup_text) = match options.format {
            FilterFormat::Standard => {
                let (text, lines) = responses::apart(text);
                response_lines.push_str(&lines);
                popups_apart(text)
            }
            FilterFormat::Hosts => (text, String::new()),
        };
        set.add_filter_list(hostnames_anchored(text), options.clone());
        if !popup_text.is_empty() {
            popups.add_filter_list(hostnames_anchored(popup_text), options);
        }
    };
    for list in &setup.lists {
        match fs::read_to_string(list) {
            Ok(text) => {
                let text = if_evaluated(text);
                let options = options(&setup.folder, list, &text);
                add(text, options);
            }
            Err(error) => crate::log!("browser: ad block list {}: {error}", list.display()),
        }
    }
    add(if_evaluated(setup.filters.clone()), ParseOptions::default());
    let mut rules = Rules::new_with_filter_set(set);
    rules.use_resources(resources(&setup.folder));
    Engine {
        rules,
        popups: Rules::new_with_filter_set(popups),
        responses: responses_of(&response_lines),
        stand_ins: responses::stand_in_urls(&stand_ins(&setup.folder)),
    }
}

/// The response-time filters of `lines`; those it can't honour are named in its unit tests.
fn responses_of(lines: &str) -> responses::Responses {
    let (responses, not_honoured) = responses::Responses::of(lines);
    if !not_honoured.is_empty() {
        crate::log!(
            "browser: ad block: {} response-time filters not honoured",
            not_honoured.len()
        );
    }
    responses
}

/// What filters name in uBlock Origin's `folder`: its stand-ins and its scriptlets.
fn resources(folder: &Path) -> Vec<Resource> {
    let mut resources = stand_ins(folder);
    resources.extend(scriptlets::scriptlets(folder));
    resources
}

/// uBlock Origin's stand-ins, the files in its web_accessible_resources its
/// js/redirect-resources.js names, with their aliases, which filters' `$redirect` name.
pub fn stand_ins(folder: &Path) -> Vec<Resource> {
    let table = fs::read_to_string(folder.join("js").join("redirect-resources.js"));
    let files = folder.join("web_accessible_resources");
    (table.as_deref().map(redirect_table).unwrap_or_default())
        .into_iter()
        .filter_map(|(name, aliases)| {
            let content = fs::read(files.join(&name)).ok()?;
            Some(Resource {
                kind: ResourceType::Mime(MimeType::from_extension(&name)),
                name,
                aliases,
                content: BASE64_STANDARD.encode(content),
                dependencies: Vec::new(),
                permission: PermissionMask::default(),
            })
        })
        .collect()
}

/// The names and aliases in uBlock Origin's redirect-resources.js, a map of entries such as
/// `[ '1x1.gif', { alias: '1x1-transparent.gif', data: 'blob' } ]`, whose alias may be a list.
fn redirect_table(source: &str) -> Vec<(String, Vec<String>)> {
    let quoted = |text: &str| -> Vec<String> {
        text.split('\'')
            .skip(1)
            .step_by(2)
            .map(str::to_owned)
            .collect()
    };
    let mut table = Vec::new();
    for entry in source.split("\n    [ ").skip(1) {
        let Some(name) = quoted(entry).into_iter().next() else {
            continue;
        };
        let body = &entry[..entry.find("} ]").unwrap_or(entry.len())];
        let aliases = match body.find("alias:") {
            Some(at) => {
                let rest = body[at + "alias:".len()..].trim_start();
                let end = match rest.starts_with('[') {
                    true => rest.find(']').map_or(rest.len(), |end| end + 1),
                    false => rest.find(',').unwrap_or(rest.len()),
                };
                quoted(&rest[..end])
            }
            None => Vec::new(),
        };
        table.push((name, aliases));
    }
    table
}

/// What the engine does with the request for `url`, of `kind` as filters name it, from the page at
/// `source`. A "popup" is a page `source` opened, which only the popups' filters close.
pub fn check(engine: &Engine, url: &str, source: &str, kind: &str) -> Verdict {
    if kind == "popup" {
        let Ok(request) = Request::new(url, source, "other", "GET") else {
            return Verdict::Allow;
        };
        return match engine.popups.check_network_request(&request).should_block() {
            true => Verdict::Block,
            false => Verdict::Allow,
        };
    }
    let Ok(request) = Request::new(url, source, kind, "GET") else {
        return Verdict::Allow;
    };
    let result = engine.rules.check_network_request(&request);
    if result.should_block() {
        return match result.redirect {
            Some(redirect) => Verdict::Redirect(redirect),
            None => Verdict::Block,
        };
    }
    if let Some(url) = result.rewritten_url {
        return Verdict::Rewrite(url);
    }
    if engine.responses.is_empty() {
        return Verdict::Allow;
    }
    let excepted = || {
        (engine.rules)
            .check_network_request_subset(&request, true, false)
            .exception
            .is_some()
    };
    match engine
        .responses
        .at_request(&asked(&request, url, source, kind), excepted)
    {
        None => Verdict::Allow,
        Some(responses::AtRequest::Ask(names)) => Verdict::AtResponse(names),
        Some(responses::AtRequest::Block(stand_in)) => {
            match stand_in.and_then(|name| engine.stand_ins.get(&name)) {
                Some(data) => Verdict::Redirect(data.clone()),
                None => Verdict::Block,
            }
        }
    }
}

/// The request for `url` as the response-time filters read it.
fn asked<'a>(request: &Request, url: &'a str, source: &str, kind: &'a str) -> responses::Asked<'a> {
    let host = responses::host_of(url);
    responses::Asked {
        address: responses::address_of(url, &host),
        url,
        kind,
        host,
        source_host: responses::host_of(source),
        third_party: request.is_third_party,
    }
}

/// Whether a response to the request for `url`, which `check` said to ask about, is blocked:
/// one sent for `method` with `headers`, lines of "name: value" of those `check` named.
pub fn at_response(
    engine: &Engine,
    url: &str,
    source: &str,
    kind: &str,
    method: &str,
    headers: &str,
) -> bool {
    let Ok(request) = Request::new(url, source, kind, method) else {
        return false;
    };
    let excepted = || {
        (engine.rules)
            .check_network_request_subset(&request, true, false)
            .exception
            .is_some()
    };
    (engine.responses).at_response(
        &asked(&request, url, source, kind),
        method,
        headers,
        excepted,
    )
}

/// What the page at `url` hides and runs from the start, as bc_host's ad_block_page writes it: a
/// JSON object of its "hide" selectors, its "styles", each a selector and the declarations for
/// it, the "exceptions" to generic hiding, its scriptlets' "script", the "isolated" one that runs
/// its procedural filters, and whether "generic" hiding applies.
pub fn page(engine: &Engine, url: &str) -> String {
    let resources = engine.rules.url_cosmetic_resources(url);
    let sorted = |set: std::collections::HashSet<String>| {
        let mut all: Vec<String> = set.into_iter().collect();
        all.sort();
        all
    };
    let mut hide = sorted(resources.hide_selectors);
    let procedural = Procedural::of(&sorted(resources.procedural_actions));
    hide.extend(procedural.hide);
    serde_json::json!({
        "hide": hide,
        "styles": procedural.styles,
        "exceptions": sorted(resources.exceptions),
        "script": wrapped(&resources.injected_script),
        "isolated": procedural.script,
        "generic": !resources.generichide,
    })
    .to_string()
}

/// procedural.js and procedural_run.js, which match a page's procedural filters and act on what
/// they find.
const PROCEDURAL: &str = concat!(
    include_str!("ad_block/procedural.js"),
    "\n",
    include_str!("ad_block/procedural_run.js")
);

/// What a page's procedural filters need: the selectors its user style sheet hides, the styles it
/// gives, each a selector and its declarations, and the script that finds the elements, for the
/// isolated world.
#[derive(Default)]
struct Procedural {
    hide: Vec<String>,
    styles: Vec<(String, String)>,
    script: String,
}

impl Procedural {
    /// Of `filters`, adblock-rust's procedural actions, each a JSON object of a "selector" and its
    /// "action". One that only styles a plain selector, as `.ad:style(height: 0)` does, is a rule
    /// of the style sheet, as uBlock Origin makes it, and needs no script.
    fn of(filters: &[String]) -> Self {
        let mut procedural = Self::default();
        let mut scripted = Vec::new();
        // Names of the page's own, so a page can't hide its elements from them, nor find them out.
        let names = std::hash::BuildHasher::hash_one(
            &std::collections::hash_map::RandomState::new(),
            filters,
        );
        let name = |n: usize| format!("{}{n}", letters(names));
        let mut styled = serde_json::Map::new();
        for filter in filters {
            let Ok(filter) = serde_json::from_str::<serde_json::Value>(filter) else {
                continue;
            };
            let style = (filter["action"]["type"] == "style")
                .then(|| filter["action"]["arg"].as_str())
                .flatten();
            let plain = match filter["selector"].as_array().map(Vec::as_slice) {
                Some([only]) if only["type"] == "css-selector" => only["arg"].as_str(),
                _ => None,
            };
            if let (Some(style), Some(plain)) = (style, plain) {
                procedural
                    .styles
                    .push((plain.to_string(), style.to_string()));
                continue;
            }
            if let Some(style) = style.filter(|style| !styled.contains_key(*style)) {
                let attribute = name(styled.len() + 1);
                procedural
                    .styles
                    .push((format!("[{attribute}]"), style.to_string()));
                styled.insert(style.to_string(), attribute.into());
            }
            scripted.push(filter);
        }
        if !scripted.is_empty() {
            let hidden = name(0);
            procedural.hide.push(format!("[{hidden}]"));
            procedural.script = format!(
                "(() => {{\n'use strict';\n{PROCEDURAL}\nrun({}, {}, {});\n}})();\n",
                serde_json::Value::from(scripted),
                serde_json::Value::from(hidden),
                serde_json::Value::from(styled),
            );
        }
        procedural
    }
}

/// `seed` as lowercase letters, an attribute name no page has.
fn letters(mut seed: u64) -> String {
    let mut out = String::from("x");
    for _ in 0..8 {
        out.push(char::from(b'a' + (seed % 26) as u8));
        seed /= 26;
    }
    out
}

/// The page's scriptlets, `script`, in a scope of their own with the globals they share, as uBlock
/// Origin runs them, or nothing for none.
fn wrapped(script: &str) -> String {
    match script.trim().is_empty() {
        true => String::new(),
        false => format!(
            "(function() {{\n'use strict';\nconst scriptletGlobals = {{}};\n{script}\n}})();\n"
        ),
    }
}

/// The generic selectors that hide the elements with these `classes` and `ids`, but the page's
/// `exceptions` (page's), as bc_host's ad_block_hide writes them: a JSON array.
pub fn hide(engine: &Engine, classes: &[String], ids: &[String], exceptions: &[String]) -> String {
    let exceptions = exceptions.iter().cloned().collect();
    let mut selectors = engine
        .rules
        .hidden_class_id_selectors(classes, ids, &exceptions);
    selectors.sort();
    serde_json::Value::from(selectors).to_string()
}

/// What `compile` makes of the same lists: one more each time it reads them differently, so an
/// engine the last compile made isn't loaded. 2: bare hostnames anchored. 3: pop-ups' filters apart,
/// the file the two engines, the first's length before it. 4: procedural filters read as such.
/// 5: `!#if` conditions evaluated as uBlock Origin on Chromium does. 6: response-time filters apart,
/// their lines after the two engines, the second's length first.
const COMPILED: u32 = 6;

/// The compiled engine's name in the cache folder, for `setup` as its files are now: a change to
/// a list, the filters, adblock-rust's version or COMPILED names another.
fn cache_name(setup: &Setup) -> String {
    let mut hasher = DefaultHasher::new();
    setup.hash(&mut hasher);
    env!("CARGO_PKG_VERSION").hash(&mut hasher);
    COMPILED.hash(&mut hasher);
    for list in &setup.lists {
        let meta = fs::metadata(list).ok();
        meta.as_ref().map(|meta| meta.len()).hash(&mut hasher);
        (meta.and_then(|meta| meta.modified().ok()))
            // A file's time is std's, as fs gives it, never read from a clock.
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .hash(&mut hasher);
    }
    format!("engine-{:016x}.dat", hasher.finish())
}

/// `setup`'s engine, from the cache in `cache` when it has one, in about 20 ms, else compiled and
/// kept there, in place of the last, for the next launch.
pub fn load(setup: &Setup, cache: Option<&Path>) -> Engine {
    let start = Instant::now();
    let file = cache.map(|cache| cache.join(cache_name(setup)));
    if let Some(mut engine) = file.as_ref().and_then(|file| read(&fs::read(file).ok()?)) {
        engine.rules.use_resources(resources(&setup.folder));
        engine.stand_ins = responses::stand_in_urls(&stand_ins(&setup.folder));
        crate::log!(
            "browser: ad block engine loaded in {} ms",
            start.elapsed().as_millis()
        );
        return engine;
    }
    let engine = compile(setup);
    crate::log!(
        "browser: ad block engine compiled in {} ms",
        start.elapsed().as_millis()
    );
    if let (Some(cache), Some(file)) = (cache, file) {
        // The last launch's goes, so the folder keeps one.
        if let Ok(entries) = fs::read_dir(cache) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name.to_string_lossy().starts_with("engine-") {
                    _ = fs::remove_file(entry.path());
                }
            }
        }
        if fs::create_dir_all(cache).is_ok() {
            let (rules, popups) = (engine.rules.serialize(), engine.popups.serialize());
            let lines = engine.responses.lines().as_bytes();
            let mut bytes = Vec::with_capacity(16 + rules.len() + popups.len() + lines.len());
            bytes.extend_from_slice(&(rules.len() as u64).to_le_bytes());
            bytes.extend(rules);
            bytes.extend_from_slice(&(popups.len() as u64).to_le_bytes());
            bytes.extend(popups);
            bytes.extend_from_slice(lines);
            _ = fs::write(&file, bytes);
        }
    }
    engine
}

/// The engines `load` kept in `bytes`, or None for a file it didn't write.
fn read(bytes: &[u8]) -> Option<Engine> {
    fn part(bytes: &[u8]) -> Option<(&[u8], &[u8])> {
        let (length, rest) = bytes.split_first_chunk::<8>()?;
        rest.split_at_checked(usize::try_from(u64::from_le_bytes(*length)).ok()?)
    }
    let (rules_bytes, rest) = part(bytes)?;
    let (popups_bytes, lines) = part(rest)?;
    let (mut rules, mut popups) = (Rules::default(), Rules::default());
    rules.deserialize(rules_bytes).ok()?;
    popups.deserialize(popups_bytes).ok()?;
    Some(Engine {
        rules,
        popups,
        responses: responses_of(std::str::from_utf8(lines).ok()?),
        stand_ins: Default::default(),
    })
}

/// The engine the core asks, swapped whole as the setup changes, as a page's requests keep asking
/// the one they have.
#[derive(Default)]
pub struct Blocker {
    state: Mutex<State>,
    ready: Condvar,
}

#[derive(Default)]
struct State {
    /// The setup asked for last, which the engine is, or is loading.
    wanted: Option<Setup>,
    engine: Option<Arc<Engine>>,
    /// Whether a request may wait for an engine: until the first has loaded, or the wait for it
    /// has once run out.
    waits: bool,
}

/// How long the requests after launch wait for the first engine, if it doesn't come.
const FIRST_WAIT: Duration = Duration::from_secs(5);

impl Blocker {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                waits: true,
                ..Default::default()
            }),
            ready: Condvar::new(),
        }
    }

    /// Blocks with `setup` from now, once it's loaded, on a thread of its own: the next request
    /// waits for it. The same setup again changes nothing.
    pub fn use_setup(self: &Arc<Self>, setup: Setup, cache: Option<PathBuf>) {
        {
            let mut state = self.state.lock().unwrap();
            if state.wanted.as_ref() == Some(&setup) {
                return;
            }
            state.wanted = Some(setup.clone());
            state.engine = None;
        }
        let blocker = self.clone();
        std::thread::spawn(move || {
            let engine = Arc::new(load(&setup, cache.as_deref()));
            let mut state = blocker.state.lock().unwrap();
            // A later setup replaced this one while it loaded.
            if state.wanted.as_ref() == Some(&setup) {
                state.engine = Some(engine);
                state.waits = true;
                blocker.ready.notify_all();
            }
        });
    }

    /// The engine to ask now: the one loading waited for, for as long as it takes, or after launch,
    /// for the first, up to FIRST_WAIT. None lets the request go.
    pub fn engine(&self) -> Option<Arc<Engine>> {
        let mut state = self.state.lock().unwrap();
        let deadline = Instant::now() + FIRST_WAIT;
        while state.engine.is_none() && state.waits {
            let now = Instant::now();
            if now >= deadline {
                crate::log!("browser: ad block engine not ready after {FIRST_WAIT:?}");
                state.waits = false;
                break;
            }
            state = self.ready.wait_timeout(state, deadline - now).unwrap().0;
        }
        state.engine.clone()
    }
}

#[cfg(test)]
mod tests;
