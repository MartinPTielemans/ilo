//! Awake while a thread is under way (#4362): one hold for every thread at once, which keeps the
//! computer from sleeping when it's left idle, as Chrome holds one for a download, and lets the
//! display sleep. It's taken as the first thread comes to be under way and let go as the last one
//! stops, is archived or is done with, on the Agents' own changes and never on a timer. A closed
//! lid or a sleep the user asks for still sleeps.

use std::rc::Rc;

use gpui::{App, Global};

use crate::agents::task::Dot;

/// What the hold is named where the system lists it, beside the app's name: macOS's `pmset -g
/// assertions`, Windows' `powercfg /requests`, the desktop's inhibitors and `systemd-inhibit
/// --list` (the words table's XS1). The same for one thread or several, at work or waiting.
pub const REASON: &str = "Running a thread";

/// What takes and lets go of the hold: the platform layer in the browser.
pub trait Host {
    /// Takes the hold with `on`, or lets it go.
    fn hold(&self, on: bool);
}

struct Awake {
    host: Rc<dyn Host>,
    held: bool,
}

impl Global for Awake {}

/// Starts following, with `host` to take the hold. Without it, `follow` does nothing, as in the
/// preview.
pub fn open(host: Rc<dyn Host>, cx: &mut App) {
    cx.set_global(Awake { host, held: false });
}

/// Whether a thread whose card shows `dot` is under way: at work, or waiting on the user, whose
/// answer it goes on with.
pub fn under_way(dot: Dot) -> bool {
    matches!(dot, Dot::Working | Dot::Waiting)
}

/// The threads' cards' dots, as the windows are sent them: the hold is taken while one is under
/// way, and asked of the host only as that changes.
pub fn follow(dots: impl IntoIterator<Item = Dot>, cx: &mut App) {
    if !cx.has_global::<Awake>() {
        return;
    }
    let on = dots.into_iter().any(under_way);
    let state = cx.global_mut::<Awake>();
    if on == state.held {
        return;
    }
    state.held = on;
    crate::log!("browser: awake {}", if on { "held" } else { "let go" });
    state.host.hold(on);
}

#[cfg(test)]
mod tests;
