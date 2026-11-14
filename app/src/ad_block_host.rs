//! The core's questions to the browser's own ad blocker (#2111, core/native_ad_block.h), answered
//! from browser::ad_block's engine. All but ad_block_use come on the core's sequence, never the
//! main thread, so they touch nothing of GPUI's.

use super::*;
use browser::ad_block::choice::{self, Engine, Launch};
use browser::ad_block::{self, Blocker, Setup, Verdict};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, OnceLock};

/// Which ad blocker this launch runs, decided as Chromium starts.
static LAUNCH: OnceLock<Launch> = OnceLock::new();

/// As Chromium starts, from the arguments the process was `given` and the user's choice in the
/// `profile` folder: which ad blocker this launch runs. Read here alone, once a launch.
pub(crate) fn prepare(given: &[String], profile: &str) -> Launch {
    let chosen = std::fs::read_to_string(std::path::Path::new(profile).join(choice::FILE))
        .ok()
        .and_then(|text| Engine::read(&text));
    *LAUNCH.get_or_init(|| Launch::new(given, chosen, cfg!(feature = "prod")))
}

/// This launch's ad blocker, for Settings' row.
pub(crate) fn launch() -> Launch {
    LAUNCH
        .get()
        .copied()
        .unwrap_or_else(|| Launch::new(&[], None, true))
}

/// The launch window is showing.
pub(crate) fn launching() {
    // check.sh reads this line.
    log!("browser: ad blocker {} at launch", launch().engine.word());
}

/// Settings' row keeps the choice in the app's folder, and restarts as Widevine's does.
pub(crate) struct Choice;

impl choice::Host for Choice {
    fn save(&self, engine: Engine) {
        let file = std::path::Path::new(&profile_dir()).join(choice::FILE);
        match std::fs::write(file, engine.word()) {
            // check.sh reads this line.
            Ok(()) => log!("browser: ad blocker {} chosen", engine.word()),
            Err(error) => log!("browser: ad blocker {} not kept: {error}", engine.word()),
        }
    }

    fn restart(&self, cx: &mut App) {
        crate::restart_browser(cx);
    }
}

static BLOCKER: LazyLock<Arc<Blocker>> = LazyLock::new(|| Arc::new(Blocker::new()));

/// Where the compiled engine is kept for the next launch: the profile folder Chromium was given.
static CACHE: OnceLock<PathBuf> = OnceLock::new();

/// Keeps the engine's cache in the profile folder of Chromium's switches `args`, whose last
/// --user-data-dir wins, as Chromium's does.
pub(crate) fn keep_cache_in(args: &[CString]) {
    let folder =
        (args.iter().rev()).find_map(|arg| arg.to_str().ok()?.strip_prefix("--user-data-dir="));
    if let Some(folder) = folder {
        _ = CACHE.set(PathBuf::from(folder).join("Ad Blocking"));
    }
}

fn text(text: *const c_char) -> String {
    match text.is_null() {
        true => String::new(),
        false => unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned(),
    }
}

pub(crate) unsafe extern "C" fn ad_block_use(
    _: *mut c_void,
    folder: *const c_char,
    lists: *const c_char,
    filters: *const c_char,
) {
    let setup = Setup {
        folder: text(folder).into(),
        lists: (text(lists).lines())
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect(),
        filters: text(filters),
    };
    BLOCKER.use_setup(setup, CACHE.get().cloned());
}

pub(crate) unsafe extern "C" fn ad_block_request(
    _: *mut c_void,
    url: *const c_char,
    source_url: *const c_char,
    kind: *const c_char,
    write: bc::bc_writer,
    out: *mut c_void,
) -> c_int {
    let Some(engine) = BLOCKER.engine() else {
        return bc::BC_AD_BLOCK_ALLOW as c_int;
    };
    let answer = |code: u32, text: &str| {
        write_out(text, write, out);
        code as c_int
    };
    let (url, kind) = (text(url), text(kind));
    match ad_block::check(&engine, &url, &text(source_url), &kind) {
        Verdict::Allow => bc::BC_AD_BLOCK_ALLOW as c_int,
        Verdict::Block => {
            // check.sh reads this line: the core closes the tab.
            if kind == "popup" {
                browser::log!("browser: ad block closed a pop-up at {url}");
            }
            bc::BC_AD_BLOCK_BLOCK as c_int
        }
        Verdict::Redirect(stand_in) => answer(bc::BC_AD_BLOCK_REDIRECT, &stand_in),
        Verdict::Rewrite(url) => answer(bc::BC_AD_BLOCK_REWRITE, &url),
    }
}

/// Writes `answer` with the core's `write`.
fn write_out(answer: &str, write: bc::bc_writer, out: *mut c_void) {
    if let Some(write) = write {
        unsafe { write(out, answer.as_ptr().cast(), answer.len()) };
    }
}

pub(crate) unsafe extern "C" fn ad_block_page(
    _: *mut c_void,
    url: *const c_char,
    write: bc::bc_writer,
    out: *mut c_void,
) {
    // With no engine, nothing: the page hides and runs nothing of the blocker's.
    if let Some(engine) = BLOCKER.engine() {
        write_out(&ad_block::page(&engine, &text(url)), write, out);
    }
}

pub(crate) unsafe extern "C" fn ad_block_hide(
    _: *mut c_void,
    classes: *const c_char,
    ids: *const c_char,
    exceptions: *const c_char,
    write: bc::bc_writer,
    out: *mut c_void,
) {
    let Some(engine) = BLOCKER.engine() else {
        return;
    };
    let strings = |json| serde_json::from_str::<Vec<String>>(&text(json)).unwrap_or_default();
    let answer = ad_block::hide(
        &engine,
        &strings(classes),
        &strings(ids),
        &strings(exceptions),
    );
    write_out(&answer, write, out);
}
