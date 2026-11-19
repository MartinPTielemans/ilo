//! Filter lists in, WebKit content-blocking rules out: `ilo-content-rules OUT LIST...` writes
//! OUT/rules-N.json and prints one line a file, `rules-N.json COUNT`.
//!
//! Brave's adblock-rust does the conversion, the engine the desktop blocks with (app/src/ad_block.rs).
//! What's here is what WebKit's rule lists need around it:
//! - A rule list holds at most 150,000 rules and an exception only reaches the rules before it in
//!   its own list, so the rules go out in several lists, each ending with every exception.
//! - An exception that matches a page's own address would lift the hiding rules too, so those go
//!   in lists of their own, after nothing.
//! - A hostname before `##` means its subdomains too, which WebKit writes `*host`, and WebKit has
//!   no way to write an entity such as `google.*`.
//! - `host#@#selector` lifts a generic `##selector` there, which WebKit writes as that rule's
//!   unless-domain.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use adblock::content_blocking::{CbRule, CbType};
use adblock::lists::{FilterFormat, FilterSet, ParseOptions};

/// Rules a list, under WebKit's 150,000 and small enough to compile in a few seconds on a phone.
const BLOCKS_A_LIST: usize = 30_000;
const HIDES_A_LIST: usize = 40_000;

/// What holds on the phone of a list's `!#if` tokens, as uBlock Origin's preparser names them: the
/// lists are read as uBlock Origin's, on a phone, in WebKit. Anything else is false; an unknown
/// token keeps its lines, as app/src/ad_block.rs's `if_evaluated` does, which this follows.
const IF_HOLDS: &[&str] = &["ext_ublock", "env_mobile", "env_safari"];
const IF_KNOWN: &[&str] = &[
    "ext_ublock", "ext_ubol", "ext_devbuild", "ext_abp", "env_brave", "env_chromium", "env_edge",
    "env_firefox", "env_legacy", "env_mobile", "env_mv3", "env_safari", "false", "adguard",
    "adguard_app_android", "adguard_app_cli", "adguard_app_ios", "adguard_app_mac",
    "adguard_app_windows", "adguard_ext_android_cb", "adguard_ext_chromium",
    "adguard_ext_chromium_mv3", "adguard_ext_edge", "adguard_ext_firefox", "adguard_ext_opera",
    "adguard_ext_safari",
];

fn if_token(token: &str) -> Option<bool> {
    let (not, token) = match token.strip_prefix('!') {
        Some(token) => (true, token),
        None => (false, token),
    };
    if !IF_KNOWN.contains(&token) && !token.starts_with("cap_") {
        return None;
    }
    Some(IF_HOLDS.contains(&token) != not)
}

/// Tokens joined by `&&` and `||`, read left to right, in one pair of brackets or none.
fn if_holds(expression: &str) -> Option<bool> {
    let expression = expression
        .strip_prefix('(')
        .and_then(|inner| inner.strip_suffix(')'))
        .unwrap_or(expression);
    let tokens: Vec<&str> = expression.split_whitespace().collect();
    let mut result = if_token(tokens.first()?)?;
    for pair in tokens[1..].chunks(2) {
        let [operator, operand] = pair else {
            return None;
        };
        let state = if_token(operand)?;
        result = match *operator {
            "||" => result || state,
            "&&" => result && state,
            _ => return None,
        };
    }
    Some(result)
}

/// `text` without the lines its `!#if` conditions leave out on the phone.
fn if_evaluated(text: &str) -> String {
    // Each open `!#if`: whether its condition is known, and whether the branch it's in goes.
    let mut open: Vec<(bool, bool)> = Vec::new();
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let directive = line.strip_prefix("!#").and_then(|rest| {
            ["if ", "else", "endif"]
                .into_iter()
                .find_map(|word| rest.strip_prefix(word).map(|after| (word, after)))
        });
        match directive {
            None => {
                if !open.iter().any(|&(known, goes)| known && goes) {
                    out.push_str(line);
                    out.push('\n');
                }
            }
            Some(("if ", condition)) => {
                let holds = if_holds(condition.trim());
                open.push((holds.is_some(), holds == Some(false)));
            }
            Some(("else", _)) => {
                if let Some((_, goes)) = open.last_mut() {
                    *goes = !*goes;
                }
            }
            Some(_) => {
                open.pop();
            }
        }
    }
    out
}

/// Whether a list is mostly a hosts file's lines, as Peter Lowe's is.
fn is_hosts(text: &str) -> bool {
    let (mut hosts, mut all) = (0, 0);
    for line in text.lines().filter(|line| !line.is_empty() && !line.starts_with(['!', '#'])) {
        all += 1;
        if line.starts_with("127.0.0.1 ") || line.starts_with("0.0.0.0 ") {
            hosts += 1;
        }
    }
    all > 0 && hosts * 2 > all
}

fn is_cosmetic(line: &str) -> bool {
    ["##", "#@#", "#?#", "#$#", "#%#"].iter().any(|mark| line.contains(mark))
}

/// A list's text as adblock-rust's converter can use it: `from=` written `domain=`, which it
/// reads only by that name, and without the `host#@#selector` lines, which go in `unhidden`.
fn prepared(text: &str, unhidden: &mut HashMap<String, Vec<String>>) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        if let Some((hosts, selector)) = line.split_once("#@#") {
            if !hosts.is_empty() && !selector.starts_with('+') {
                let hosts = hosts.split(',').map(str::to_lowercase).filter(|host| is_host(host));
                unhidden
                    .entry(selector.to_string())
                    .or_default()
                    .extend(hosts.map(|host| format!("*{host}")));
            }
            continue;
        }
        if !is_cosmetic(line) && line.contains("from=") {
            out.push_str(&line.replace("$from=", "$domain=").replace(",from=", ",domain="));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// Whether WebKit takes `host` as a site: lowercase ASCII, and no entity's `.*`.
fn is_host(host: &str) -> bool {
    !host.is_empty()
        && !host.starts_with('.')
        && host.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b".-_".contains(&c))
}

/// A rule's sites as WebKit takes them, or None for a rule that can't be said in them. A hostname
/// means its subdomains too, written `*host`; `*` alone is every site; and an entity such as
/// `google.*` can't be written, so it's left out, and a rule for entities alone is too.
fn sites(mut rule: CbRule) -> Option<CbRule> {
    let clean = |hosts: Option<Vec<String>>| -> Option<Vec<String>> {
        let hosts: Vec<String> = hosts?
            .into_iter()
            .map(|host| host.trim_start_matches('*').to_lowercase())
            .filter(|host| is_host(host))
            .map(|host| format!("*{host}"))
            .collect();
        Some(hosts)
    };
    let every_site = rule.trigger.if_domain.as_ref().is_some_and(|hosts| hosts.iter().all(|host| host == "*"));
    rule.trigger.if_domain = if every_site { None } else { clean(rule.trigger.if_domain.take()) };
    rule.trigger.unless_domain = clean(rule.trigger.unless_domain.take()).filter(|hosts| !hosts.is_empty());
    match &rule.trigger.if_domain {
        Some(hosts) if hosts.is_empty() => None,
        _ => Some(rule),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("usage: ilo-content-rules OUT LIST..."));
    let mut set = FilterSet::new(true);
    let mut unhidden = HashMap::new();
    for list in args {
        let text = fs::read_to_string(&list).unwrap_or_else(|error| panic!("{list}: {error}"));
        let text = prepared(&if_evaluated(&text), &mut unhidden);
        let format = if is_hosts(&text) { FilterFormat::Hosts } else { FilterFormat::Standard };
        set.add_filter_list(text, ParseOptions { format, ..ParseOptions::default() });
    }
    let (rules, _) = set.into_content_blocking().expect("the set keeps its lines");

    let (mut blocks, mut exceptions, mut hides) = (Vec::new(), Vec::new(), Vec::new());
    for mut rule in rules.into_iter().filter_map(sites) {
        match rule.action.typ {
            CbType::IgnorePreviousRules => exceptions.push(rule),
            CbType::CssDisplayNone => {
                let lifted = rule.action.selector.as_ref().and_then(|selector| unhidden.get(selector));
                match (lifted, &mut rule.trigger.if_domain) {
                    (Some(hosts), None) if !hosts.is_empty() => rule.trigger.unless_domain.get_or_insert_default().extend(hosts.iter().cloned()),
                    // One written for some sites and lifted on one of them.
                    (Some(hosts), Some(sites)) => sites.retain(|site| !hosts.contains(site)),
                    _ => {}
                }
                if rule.trigger.if_domain.as_ref().is_none_or(|sites| !sites.is_empty()) {
                    hides.push(rule);
                }
            }
            _ => blocks.push(rule),
        }
    }

    fs::create_dir_all(&out).expect("the folder for the rules");
    let mut lists: Vec<Vec<&CbRule>> = Vec::new();
    for chunk in blocks.chunks(BLOCKS_A_LIST) {
        lists.push(chunk.iter().chain(&exceptions).collect());
    }
    for chunk in hides.chunks(HIDES_A_LIST) {
        lists.push(chunk.iter().collect());
    }
    for (index, list) in lists.iter().enumerate() {
        let name = format!("rules-{index}.json");
        let json = serde_json::to_vec(list).expect("rules are JSON");
        fs::write(out.join(&name), json).expect("the rules' file");
        println!("{name} {}", list.len());
    }
    eprintln!("{} blocking, {} exceptions in each, {} hiding", blocks.len(), exceptions.len(), hides.len());
}
