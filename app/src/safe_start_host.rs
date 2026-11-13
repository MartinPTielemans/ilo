//! Safe start's host (browser::safe_start, #2345): the launch mark in the app's folder, the core's
//! switch that holds the extensions, the Boosts held, and Turn Back On. Agents get none of it: the
//! MCP server has no tool that starts or ends it, and keeps running in it.

use std::sync::OnceLock;
use std::time::Instant;

use browser::safe_start::{self, Mark};

use super::*;

/// This launch's mark, decided as Chromium starts.
static LAUNCH: OnceLock<Mark> = OnceLock::new();
/// When the launch window showed, from which the first minute counts.
static SHOWN: OnceLock<Instant> = OnceLock::new();

fn path() -> String {
    format!("{}/{}", profile_dir(), safe_start::MARK)
}

fn write(mark: Mark) {
    if let Err(error) = std::fs::write(path(), mark.line()) {
        log!("browser: launch mark not written: {error}");
    }
}

/// Whether this launch holds extensions and Boosts.
pub(crate) fn held() -> bool {
    LAUNCH.get().is_some_and(|mark| mark.is_safe())
}

/// As Chromium starts: this launch's mark, after the last one's, and the core's switch saying
/// whether it's safe. Only read here: a second copy of the browser reads it too before it hands
/// its links to the first and ends, and mustn't count.
pub(crate) fn prepare() -> String {
    let last = std::fs::read_to_string(path())
        .ok()
        .and_then(|text| Mark::read(&text));
    let mark = *LAUNCH.get_or_init(|| Mark::after(last));
    format!("{}={}", safe_start::SWITCH, u8::from(mark.is_safe()))
}

/// The launch window is showing, past Chromium's check for another copy: the mark goes down, and
/// a safe launch holds the Boosts before any profile's are read.
pub(crate) fn launching() {
    _ = SHOWN.set(Instant::now());
    let Some(&mark) = LAUNCH.get() else {
        return;
    };
    write(mark);
    let Mark::Launching(crashes) = mark else {
        return;
    };
    // check.sh reads this line.
    log!(
        "browser: safe start {} after {crashes} crashes at launch",
        if mark.is_safe() { "on" } else { "off" }
    );
    if mark.is_safe() {
        browser::boosts::with(|boosts| boosts.set_held(true));
    }
}

/// Once the app has started: a minute after the window showed, the launch is past its launch, and
/// a crash from then on counts nothing. One timer, and none after it.
pub(crate) fn start(cx: &mut App) {
    let since = SHOWN.get().map_or(Duration::ZERO, Instant::elapsed);
    let after = safe_start::AT_LAUNCH.saturating_sub(since);
    cx.spawn(async move |cx| {
        cx.background_executor().timer(after).await;
        cx.update(|_| {
            write(Mark::Running);
            log!("browser: launch mark running");
        });
    })
    .detach();
}

/// Chromium is quitting, which ends a safe start: the next launch is a normal one.
pub(crate) fn stopping() {
    write(Mark::Quit);
}
