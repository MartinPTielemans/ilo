//! The browser's own ad blocker (#2111): Brave's adblock-rust on the lists uBlock Origin comes with,
//! in place of uBlock Origin with `--ad-block-engine=native`. The core finds the requests pages make
//! and asks the app about each (core/native_ad_block.h), which `ad_block_host.rs` answers from the
//! engine here, on the core's sequence, never the main thread.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime};

use adblock::Engine;
use adblock::lists::{FilterFormat, FilterSet, ParseOptions};
use adblock::request::Request;
use adblock::resources::{MimeType, PermissionMask, Resource, ResourceType};
use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;

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

/// What to do with a request (BC_AD_BLOCK_).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Block,
    /// Answered in its place with this data: URL.
    Redirect(String),
    /// Sent to this address instead.
    Rewrite(String),
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

/// Compiles `setup`'s lists and the user's filters, about 0.25 s for the lists uBlock Origin turns
/// on, and gives it uBlock Origin's stand-ins and scriptlets. A list that can't be read is left out.
pub fn compile(setup: &Setup) -> Engine {
    let mut set = FilterSet::new(false);
    for list in &setup.lists {
        match fs::read_to_string(list) {
            Ok(text) => {
                let options = options(&setup.folder, list, &text);
                set.add_filter_list(text, options);
            }
            Err(error) => crate::log!("browser: ad block list {}: {error}", list.display()),
        }
    }
    set.add_filter_list(setup.filters.clone(), ParseOptions::default());
    let mut engine = Engine::new_with_filter_set(set);
    engine.use_resources(resources(&setup.folder));
    engine
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
/// `source`.
pub fn check(engine: &Engine, url: &str, source: &str, kind: &str) -> Verdict {
    let Ok(request) = Request::new(url, source, kind, "GET") else {
        return Verdict::Allow;
    };
    let result = engine.check_network_request(&request);
    if result.should_block() {
        return match result.redirect {
            Some(redirect) => Verdict::Redirect(redirect),
            None => Verdict::Block,
        };
    }
    match result.rewritten_url {
        Some(url) => Verdict::Rewrite(url),
        None => Verdict::Allow,
    }
}

/// What the page at `url` hides and runs from the start, as bc_host's ad_block_page writes it: a
/// JSON object of its "hide" selectors, its "procedural" filters, the "exceptions" to generic
/// hiding, its scriptlets' "script", and whether "generic" hiding applies.
pub fn page(engine: &Engine, url: &str) -> String {
    let resources = engine.url_cosmetic_resources(url);
    let sorted = |set: std::collections::HashSet<String>| {
        let mut all: Vec<String> = set.into_iter().collect();
        all.sort();
        all
    };
    serde_json::json!({
        "hide": sorted(resources.hide_selectors),
        "procedural": sorted(resources.procedural_actions),
        "exceptions": sorted(resources.exceptions),
        "script": wrapped(&resources.injected_script),
        "generic": !resources.generichide,
    })
    .to_string()
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
    let mut selectors = engine.hidden_class_id_selectors(classes, ids, &exceptions);
    selectors.sort();
    serde_json::Value::from(selectors).to_string()
}

/// The compiled engine's name in the cache folder, for `setup` as its files are now: a change to
/// a list, the filters or adblock-rust's version names another.
fn cache_name(setup: &Setup) -> String {
    let mut hasher = DefaultHasher::new();
    setup.hash(&mut hasher);
    env!("CARGO_PKG_VERSION").hash(&mut hasher);
    for list in &setup.lists {
        let meta = fs::metadata(list).ok();
        meta.as_ref().map(|meta| meta.len()).hash(&mut hasher);
        (meta.and_then(|meta| meta.modified().ok()))
            .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
            .hash(&mut hasher);
    }
    format!("engine-{:016x}.dat", hasher.finish())
}

/// `setup`'s engine, from the cache in `cache` when it has one, in about 20 ms, else compiled, and
/// whether it was compiled, for `keep` to write once it's in use.
pub fn load(setup: &Setup, cache: Option<&Path>) -> (Engine, bool) {
    let start = Instant::now();
    let file = cache.map(|cache| cache.join(cache_name(setup)));
    if let Some(bytes) = file.as_ref().and_then(|file| fs::read(file).ok()) {
        let mut engine = Engine::default();
        if engine.deserialize(&bytes).is_ok() {
            engine.use_resources(resources(&setup.folder));
            crate::log!(
                "browser: ad block engine loaded in {} ms",
                start.elapsed().as_millis()
            );
            return (engine, false);
        }
    }
    let engine = compile(setup);
    crate::log!(
        "browser: ad block engine compiled in {} ms",
        start.elapsed().as_millis()
    );
    (engine, true)
}

/// Keeps `engine`, compiled for `setup`, in `cache` for the next launch, in place of the last.
pub fn keep(setup: &Setup, cache: &Path, engine: &Engine) {
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
        _ = fs::write(cache.join(cache_name(setup)), engine.serialize());
    }
}

/// The engine the core asks, swapped whole as the setup changes, as a page's requests keep asking
/// the one they have. While a new setup loads, the last engine answers: a list change mustn't hold
/// a page's requests, as uBlock Origin's rebuild holds them for 1 to 2.5 s (#2513).
#[derive(Default)]
pub struct Blocker {
    state: Mutex<State>,
    ready: Condvar,
    /// Held while a compiled engine is written, so an older setup's load, done last, can't write
    /// over the newer's.
    keeping: Mutex<()>,
    /// Held by a test to keep a loaded engine from coming in.
    #[cfg(test)]
    swap: Mutex<()>,
}

#[derive(Default)]
struct State {
    /// The setup asked for last, which the engine is, or is loading.
    wanted: Option<Setup>,
    /// The last setup's engine to load, which answers until the next is in.
    engine: Option<Arc<Engine>>,
    /// Whether a request may wait for the first engine: until it has loaded, or the wait for it
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
            ..Default::default()
        }
    }

    /// Blocks with `setup` once it's loaded, on a thread of its own, and until then with the engine
    /// it has. A compiled engine is written to `cache` after it's in, off the requests' way. The
    /// same setup again changes nothing.
    pub fn use_setup(self: &Arc<Self>, setup: Setup, cache: Option<PathBuf>) {
        {
            let mut state = self.state.lock().unwrap();
            if state.wanted.as_ref() == Some(&setup) {
                return;
            }
            state.wanted = Some(setup.clone());
        }
        let blocker = self.clone();
        std::thread::spawn(move || {
            let (engine, compiled) = load(&setup, cache.as_deref());
            let engine = Arc::new(engine);
            #[cfg(test)]
            drop(blocker.swap.lock().unwrap());
            // A later setup replaced this one while it loaded.
            if !blocker.swap_in(&setup, &engine) {
                return;
            }
            if let (true, Some(cache)) = (compiled, cache) {
                let _keeping = blocker.keeping.lock().unwrap();
                if blocker.state.lock().unwrap().wanted.as_ref() == Some(&setup) {
                    keep(&setup, &cache, &engine);
                }
            }
        });
    }

    /// Puts `engine` in for the requests from now, if `setup` is still the one wanted.
    fn swap_in(&self, setup: &Setup, engine: &Arc<Engine>) -> bool {
        let mut state = self.state.lock().unwrap();
        if state.wanted.as_ref() != Some(setup) {
            return false;
        }
        state.engine = Some(engine.clone());
        self.ready.notify_all();
        true
    }

    /// The engine to ask now: the last to load, or after launch, the first, waited for up to
    /// FIRST_WAIT. None lets the request go.
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
