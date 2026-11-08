//! Brave's adblock-rust on the lists ilo's uBlock Origin ships, measured as uBO's engine is by
//! ubo.mjs, so the two can be set side by side: what compiling and loading cost in time and
//! memory, what a request match costs, what each blocks, and which of the lists' lines it can't
//! use. Each step runs in a process of its own, so one step's memory isn't counted in another's.

use std::fs;
use std::path::Path;
use std::time::Instant;

use adblock::Engine;
use adblock::lists::{FilterFormat, FilterSet, ParseOptions, ParsedLine, parse_filter};
use adblock::request::Request;
use adblock::resources::{PermissionMask, Resource};

mod memory;

/// uBlock Origin's own lists may use its trusted scriptlets; the others may not.
fn permissions(list: &str) -> PermissionMask {
    if list.starts_with("ublock-") {
        PermissionMask::from_bits(1)
    } else {
        PermissionMask::default()
    }
}

/// How to read a list: uBO reads a hosts file's lines, as Peter Lowe's list is, as `||host^`
/// wherever they are, where adblock-rust has to be told the list is one.
fn options(name: &str, text: &str) -> ParseOptions {
    let lines = text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with(['!', '#']));
    let (mut hosts, mut all) = (0, 0);
    for line in lines {
        all += 1;
        if line.starts_with("127.0.0.1 ") || line.starts_with("0.0.0.0 ") {
            hosts += 1;
        }
    }
    ParseOptions {
        format: if hosts * 2 > all {
            FilterFormat::Hosts
        } else {
            FilterFormat::Standard
        },
        permissions: permissions(name),
        ..Default::default()
    }
}

/// The lists in `dir`, by name, in name order.
fn lists(dir: &Path) -> Vec<(String, String)> {
    let mut lists: Vec<_> = fs::read_dir(dir)
        .expect("the lists' folder")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "txt"))
        .map(|path| {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            (name, fs::read_to_string(&path).unwrap())
        })
        .collect();
    lists.sort();
    lists
}

fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.
}

/// Parses and compiles the lists, as a first launch would, and writes the engine's serialized
/// form, as its cache.
fn compile(dir: &Path, out: &Path) {
    let lists = lists(dir);
    let before = memory::now();
    let start = Instant::now();
    let mut set = FilterSet::new(false);
    for (name, text) in &lists {
        set.add_filter_list(text.clone(), options(name, text));
    }
    let engine = Engine::new_with_filter_set(set);
    let compiled = ms(start);
    let held = memory::now();
    let start = Instant::now();
    let bytes = engine.serialize();
    let serialized = ms(start);
    fs::write(out, &bytes).unwrap();
    let peak = memory::peak();
    println!(
        "{}",
        serde_json::json!({
            "step": "compile",
            "lists": lists.len(),
            "compile_ms": compiled,
            "serialize_ms": serialized,
            "serialized_kb": bytes.len() / 1024,
            "memory_before": before,
            "memory_held": held,
            "memory_peak": peak,
        })
    );
    drop(engine);
}

/// Loads the engine from its cache, as each later launch would.
fn load(cache: &Path) -> Engine {
    let bytes = fs::read(cache).unwrap();
    let mut engine = Engine::default();
    engine.deserialize(&bytes).expect("the cache");
    engine
}

fn load_step(cache: &Path) {
    let before = memory::now();
    let start = Instant::now();
    let engine = load(cache);
    let loaded = ms(start);
    let held = memory::now();
    println!(
        "{}",
        serde_json::json!({
            "step": "load",
            "load_ms": loaded,
            "memory_before": before,
            "memory_held": held,
            "memory_peak": memory::peak(),
        })
    );
    drop(engine);
}

/// The request's type as uBlock Origin names it, from the corpus's. ubo.mjs maps them the same.
/// The corpus gives each document itself as its frame, so a frame can't be told from a page, and
/// nearly all of them are an ad's or a tracker's frames: they're taken as frames.
fn request_type(cpt: &str) -> &'static str {
    match cpt {
        "document" => "sub_frame",
        "xhr" | "fetch" => "xmlhttprequest",
        "image" => "image",
        "script" => "script",
        "stylesheet" => "stylesheet",
        "font" => "font",
        "media" => "media",
        "websocket" => "websocket",
        "ping" | "beacon" => "ping",
        _ => "other",
    }
}

/// Matches each request in `requests`, a JSON object a line, and writes a line a request: 1 when
/// it's blocked, 2 when an exception let it through, 0 when nothing matched, as uBO's
/// matchRequest answers, then a tab and "redirect" when it's answered with a stand-in, which needs
/// uBO's resources (resources.mjs).
fn matches(cache: &Path, requests: &Path, out: &Path, resources: Option<&Path>) {
    let mut engine = load(cache);
    if let Some(resources) = resources {
        let resources: Vec<Resource> =
            serde_json::from_slice(&fs::read(resources).unwrap()).unwrap();
        engine.use_resources(resources);
    }
    let requests: Vec<(String, String, &'static str)> = fs::read_to_string(requests)
        .unwrap()
        .lines()
        .map(|line| {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            let (url, frame) = (
                row["url"].as_str().unwrap(),
                row["frameUrl"].as_str().unwrap(),
            );
            let cpt = request_type(row["cpt"].as_str().unwrap());
            (url.to_string(), frame.to_string(), cpt)
        })
        .collect();
    let mut times = Vec::with_capacity(requests.len());
    let mut lines = String::new();
    let (mut blocked, mut excepted, mut redirected) = (0, 0, 0);
    // Twice: the first warms the regexes and caches, as a page's first requests do; the second
    // is timed.
    for pass in 0..2 {
        for (url, frame, cpt) in &requests {
            let start = Instant::now();
            let result = Request::new(url, frame, cpt, "GET")
                .map(|request| engine.check_network_request(&request));
            let took = start.elapsed().as_nanos() as u64;
            if pass == 0 {
                continue;
            }
            times.push(took);
            let (verdict, redirect) = match &result {
                Ok(result) if result.should_block() => {
                    blocked += 1;
                    if result.redirect.is_some() {
                        redirected += 1;
                    }
                    (1, result.redirect.is_some())
                }
                Ok(result) if result.exception.is_some() => {
                    excepted += 1;
                    (2, false)
                }
                _ => (0, false),
            };
            lines.push_str(&format!(
                "{verdict}\t{}\n",
                if redirect { "redirect" } else { "" }
            ));
        }
    }
    fs::write(out, lines).unwrap();
    let total: u64 = times.iter().sum();
    times.sort_unstable();
    let at = |q: f64| times[((times.len() - 1) as f64 * q) as usize] as f64 / 1000.;
    println!(
        "{}",
        serde_json::json!({
            "step": "match",
            "requests": times.len(),
            "blocked": blocked,
            "excepted": excepted,
            "redirected": redirected,
            "mean_us": total as f64 / times.len() as f64 / 1000.,
            "p50_us": at(0.5),
            "p99_us": at(0.99),
            "max_us": at(1.),
        })
    );
}

/// Each line of the lists adblock-rust can't use, counted by the kind of filter it is, with a few
/// of each kind.
fn syntax(dir: &Path) {
    let mut kinds: std::collections::BTreeMap<String, (usize, Vec<String>)> = Default::default();
    let (mut network, mut cosmetic, mut scriptlets, mut failed) = (0, 0, 0, 0);
    for (name, text) in lists(dir) {
        let opts = options(&name, &text);
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty()
                || line.starts_with('!')
                || line.starts_with("[Adblock")
                || (line.starts_with('#') && !line.starts_with("##"))
            {
                continue;
            }
            match parse_filter(line, false, opts) {
                Ok(ParsedLine::Network(_)) => network += 1,
                Ok(ParsedLine::Cosmetic(_)) if line.contains("+js(") => scriptlets += 1,
                Ok(ParsedLine::Cosmetic(_)) => cosmetic += 1,
                Err(error) => {
                    failed += 1;
                    let entry = kinds.entry(kind(line, &error.to_string())).or_default();
                    entry.0 += 1;
                    if entry.1.len() < 3 {
                        entry.1.push(format!("{name}: {line}"));
                    }
                }
            }
        }
    }
    let mut kinds: Vec<_> = kinds.into_iter().collect();
    kinds.sort_by(|a, b| b.1.0.cmp(&a.1.0));
    println!(
        "{}",
        serde_json::json!({
            "step": "syntax",
            "network": network,
            "cosmetic": cosmetic,
            "scriptlets": scriptlets,
            "unusable": failed,
            "unusable_by_kind": kinds.into_iter().map(|(kind, (count, examples))| {
                serde_json::json!({ "kind": kind, "count": count, "examples": examples })
            }).collect::<Vec<_>>(),
        })
    );
}

/// The uBO feature a line adblock-rust can't use relies on, as near as its text says.
fn kind(line: &str, error: &str) -> String {
    const OPTIONS: &[&str] = &[
        "removeparam",
        "csp",
        "header",
        "replace",
        "urlskip",
        "uritransform",
        "permissions",
        "redirect",
        "redirect-rule",
        "to",
        "from",
        "denyallow",
        "method",
        "ipaddress",
        "strict1p",
        "strict3p",
        "doc",
        "popup",
        "popunder",
        "elemhide",
        "generichide",
        "specifichide",
        "inline-script",
        "inline-font",
        "mp4",
        "empty",
        "badfilter",
        "match-case",
        "all",
        "cname",
        "important",
        "responseheader",
        "urltransform",
        "reason",
    ];
    if line.contains("##^") || line.contains("#@#^") {
        return "HTML filtering (##^)".into();
    }
    if line.contains("#?#")
        || line.contains(":has-text(")
        || line.contains(":matches-")
        || line.contains(":upward(")
        || line.contains(":xpath(")
        || line.contains(":remove(")
        || line.contains(":style(")
    {
        return "procedural cosmetic".into();
    }
    if line.contains("##+js(") || line.contains("#@#+js(") {
        return "scriptlet".into();
    }
    if line.contains("##") || line.contains("#@#") || line.contains("#$#") {
        return format!("cosmetic: {error}");
    }
    if let Some(at) = line.rfind('$') {
        for option in line[at + 1..].split(',') {
            let option = option
                .trim_start_matches('~')
                .split('=')
                .next()
                .unwrap_or("");
            if OPTIONS.contains(&option) {
                return format!("network ${option}: {error}");
            }
        }
    }
    format!("network: {error}")
}

/// What adblock-rust gives a page at `url` to hide and run, with uBO's resources loaded.
fn cosmetic(cache: &Path, resources: &Path, urls: &[String]) {
    let mut engine = load(cache);
    let resources: Vec<Resource> = serde_json::from_slice(&fs::read(resources).unwrap()).unwrap();
    let count = resources.len();
    engine.use_resources(resources);
    for url in urls {
        let start = Instant::now();
        let found = engine.url_cosmetic_resources(url);
        println!(
            "{}",
            serde_json::json!({
                "step": "cosmetic",
                "url": url,
                "resources": count,
                "ms": ms(start),
                "hide_selectors": found.hide_selectors.len(),
                "procedural_actions": found.procedural_actions.len(),
                "injected_script_bytes": found.injected_script.len(),
                "generichide": found.generichide,
            })
        );
    }
}

/// Which filter decided each request, a URL, its frame's URL and its type, three arguments each.
fn explain(dir: &Path, requests: &[String]) {
    let mut set = FilterSet::new(true);
    for (name, text) in lists(dir) {
        set.add_filter_list(text.clone(), options(&name, &text));
    }
    let engine = Engine::new_with_filter_set(set);
    for request in requests.chunks(3) {
        let result = engine.check_network_request(
            &Request::new(&request[0], &request[1], &request[2], "GET").unwrap(),
        );
        println!(
            "{} {} {} <= {:?} except {:?}",
            u8::from(result.should_block()),
            request[2],
            &request[0][..request[0].len().min(80)],
            result.filter,
            result.exception
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |i: usize| Path::new(&args[i]);
    match args.get(1).map(String::as_str) {
        Some("compile") => compile(arg(2), arg(3)),
        Some("load") => load_step(arg(2)),
        Some("match") => matches(arg(2), arg(3), arg(4), args.get(5).map(Path::new)),
        Some("syntax") => syntax(arg(2)),
        Some("cosmetic") => cosmetic(arg(2), arg(3), &args[4..]),
        Some("explain") => explain(arg(2), &args[3..]),
        _ => {
            eprintln!(
                "adblock-spike compile LISTS ENGINE | load ENGINE | match ENGINE REQUESTS OUT [RESOURCES] | \
                 syntax LISTS | cosmetic ENGINE RESOURCES URL... | explain LISTS (URL FRAME TYPE)..."
            );
            std::process::exit(2);
        }
    }
}
