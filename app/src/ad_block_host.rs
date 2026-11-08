//! The core's questions to the browser's own ad blocker (#2111, core/native_ad_block.h), answered
//! from browser::ad_block's engine. All but ad_block_use come on the core's sequence, never the
//! main thread, so they touch nothing of GPUI's.

use super::*;
use browser::ad_block::{self, Blocker, Setup, Verdict};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, OnceLock};

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
        if let Some(write) = write {
            unsafe { write(out, text.as_ptr().cast(), text.len()) };
        }
        code as c_int
    };
    match ad_block::check(&engine, &text(url), &text(source_url), &text(kind)) {
        Verdict::Allow => bc::BC_AD_BLOCK_ALLOW as c_int,
        Verdict::Block => bc::BC_AD_BLOCK_BLOCK as c_int,
        Verdict::Redirect(stand_in) => answer(bc::BC_AD_BLOCK_REDIRECT, &stand_in),
        Verdict::Rewrite(url) => answer(bc::BC_AD_BLOCK_REWRITE, &url),
    }
}
