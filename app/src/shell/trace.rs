//! Record a Trace (docs/ux-spec.md › Record a Trace): a Chromium trace of the whole browser, which
//! the user starts from Help › Troubleshooting or the command bar, sees recording in a toast with
//! Stop that stays until they stop it, and saves where they pick. Nothing records before it's
//! started, and the trace goes nowhere but the file. The user's alone: agents get no tool for it.

use std::{path::PathBuf, rc::Rc, time::Duration};

use gpui::{App, Global};

use super::{Browser, commands::ShellHandle};

pub(super) const RECORD: &str = "Record a Trace";
pub(super) const STOP_RECORDING: &str = "Stop Recording Trace";
/// The toast's words while a trace records, and its button's.
pub const RECORDING: &str = "Recording a trace";
pub const STOP: &str = "Stop";
const NOT_STARTED: &str = "Couldn't start a trace";
const NOT_SAVED: &str = "Couldn't save the trace";
const NOTE_TIME: Duration = Duration::from_secs(3);

/// A trace is recording.
struct Recording;

impl Global for Recording {}

pub(super) fn recording(cx: &App) -> bool {
    cx.has_global::<Recording>()
}

/// What turns the trace the other way, for the menu and the command bar.
pub(super) fn title(recording: bool) -> &'static str {
    if recording { STOP_RECORDING } else { RECORD }
}

/// Starts a trace, or stops the one recording.
pub(super) fn toggle(browser: Rc<dyn Browser>, cx: &mut App) {
    if recording(cx) {
        return stop(&*browser, cx);
    }
    if !browser.start_trace() {
        // check.sh reads this line.
        log!("browser: trace not started");
        return browser.toast(NOT_STARTED, None, NOTE_TIME, cx);
    }
    cx.set_global(Recording);
    // check.sh reads this line.
    log!("browser: trace recording");
    let stopping = browser.clone();
    let stop = Rc::new(move |cx: &mut App| {
        stop(&*stopping, cx);
        // The menu bar's item is the key window's to name.
        if let Some(shell) = cx
            .try_global::<ShellHandle>()
            .map(|handle| handle.0.clone())
        {
            shell.update(cx, |shell, cx| shell.update_menus(cx)).ok();
        }
    });
    browser.trace_note(Some(stop), cx);
}

fn stop(browser: &dyn Browser, cx: &mut App) {
    if !recording(cx) {
        return;
    }
    cx.remove_global::<Recording>();
    browser.trace_note(None, cx);
    browser.stop_trace();
    // check.sh reads this line.
    log!("browser: trace stopped");
}

/// The trace the core recorded, as Chromium's JSON, which the user saves where they pick, or drops
/// with the panel's Cancel.
pub fn recorded(json: Option<Vec<u8>>, cx: &mut App) {
    let shell = cx
        .try_global::<ShellHandle>()
        .and_then(|handle| handle.0.upgrade());
    let Some(browser) = shell.map(|shell| shell.read(cx).browser.clone()) else {
        return;
    };
    let Some(json) = json.filter(|json| !json.is_empty()) else {
        return browser.toast(NOT_SAVED, None, NOTE_TIME, cx);
    };
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let downloads = home.join("Downloads");
    let folder = if downloads.is_dir() { downloads } else { home };
    let chosen = cx.prompt_for_new_path(&folder, Some(concat!(env!("APP_NAME"), " Trace.json")));
    // check.sh reads this line.
    log!("browser: trace save panel shown");
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(path))) = chosen.await else {
            return;
        };
        let written = cx
            .background_executor()
            .spawn({
                let path = path.clone();
                async move { std::fs::write(path, json) }
            })
            .await;
        match written {
            // check.sh reads this line.
            Ok(()) => log!("browser: trace saved {}", path.display()),
            Err(error) => {
                log!("browser: trace not saved: {error}");
                cx.update(|cx| browser.toast(NOT_SAVED, None, NOTE_TIME, cx));
            }
        }
    })
    .detach();
}
