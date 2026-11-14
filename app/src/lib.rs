//! The browser's UI, drawn with GPUI. `Browser` (main.rs) runs it on the engine, through the core's
//! C API; `preview` runs it alone with made-up tabs.

// The agents' tools are one json! list, which takes more than the default 128 to expand.
#![recursion_limit = "256"]

/// Writes a line to stderr, as `eprintln!` does, but in one write where `eprintln!` makes one for
/// each piece, so a line Chromium logs at the same time can't land inside it, where the checks
/// wouldn't find it.
#[macro_export]
#[cfg(not(target_family = "wasm"))]
macro_rules! log {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        _ = std::io::stderr().write_all(format!("{}\n", format_args!($($arg)*)).as_bytes());
    }};
}

/// The web has no stderr; the page's console is where its lines go.
#[macro_export]
#[cfg(target_family = "wasm")]
macro_rules! log {
    ($($arg:tt)*) => {
        web_sys::console::log_1(&format!($($arg)*).into())
    };
}

/// Milliseconds on macOS's uptime clock, the one Chromium's `TimeTicks` and the host's lines read,
/// so a step timed across the core, the host and the UI is one subtraction.
#[cfg(target_os = "macos")]
pub fn uptime_ms() -> f64 {
    unsafe extern "C" {
        fn clock_gettime_nsec_np(clock_id: u32) -> u64;
    }
    const CLOCK_UPTIME_RAW: u32 = 8;
    unsafe { clock_gettime_nsec_np(CLOCK_UPTIME_RAW) as f64 / 1e6 }
}

/// Milliseconds on the performance counter, the clock Chromium's `TimeTicks` reads on Windows.
#[cfg(target_os = "windows")]
pub fn uptime_ms() -> f64 {
    unsafe extern "system" {
        fn QueryPerformanceCounter(count: *mut i64) -> i32;
        fn QueryPerformanceFrequency(frequency: *mut i64) -> i32;
    }
    let (mut count, mut frequency) = (0, 0);
    unsafe {
        QueryPerformanceCounter(&mut count);
        QueryPerformanceFrequency(&mut frequency);
    }
    count as f64 * 1e3 / frequency as f64
}

/// Milliseconds on Linux's monotonic clock, the one Chromium's `TimeTicks` reads there.
#[cfg(target_os = "linux")]
pub fn uptime_ms() -> f64 {
    use std::ffi::{c_int, c_long};
    #[repr(C)]
    struct Timespec {
        tv_sec: c_long,
        tv_nsec: c_long,
    }
    unsafe extern "C" {
        fn clock_gettime(clock_id: c_int, tp: *mut Timespec) -> c_int;
    }
    const CLOCK_MONOTONIC: c_int = 1;
    let mut now = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe { clock_gettime(CLOCK_MONOTONIC, &mut now) };
    now.tv_sec as f64 * 1e3 + now.tv_nsec as f64 / 1e6
}

/// Milliseconds on the page's clock, the site demo's, which has no engine to agree with.
#[cfg(target_family = "wasm")]
pub fn uptime_ms() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map_or(0., |clock| clock.now())
}

/// The core's API, generated from core/public/browser_core.h. Here, rather than in the browser's
/// own crate, for the Windows browser's crate too (windows/).
#[allow(non_camel_case_types, dead_code)]
pub mod bc {
    include!(concat!(env!("OUT_DIR"), "/browser_core.rs"));
}

thread_local! {
    static HANDOVER_CLICK: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
}

/// The app asks a window to select a tab, which may move a page between windows. The core marks
/// the same moment, in the same call.
pub fn handover_clicked() {
    HANDOVER_CLICK.set(Some(uptime_ms()));
}

/// The lines check.sh times a page's move between windows by, as the core's are: "browser: handover
/// `stage` +<ms> ms (`detail`)", in ms since `handover_clicked`, within 3 s of it. No page content.
pub fn log_handover(stage: &str, detail: &str) {
    let Some(click) = HANDOVER_CLICK.get() else {
        return;
    };
    let since = uptime_ms() - click;
    if since <= 3000. {
        log!("browser: handover {stage} +{since:.1} ms ({detail})");
    }
}

pub mod account;
pub mod ad_block;
pub mod agent_history;
pub mod agents;
pub mod boost_editor;
pub mod boost_style;
pub mod boosts;
pub mod build_info;
pub mod capture;
pub mod child;
pub mod command_bar;
pub mod daily_count;
pub mod default_browser;
pub mod dialog;
pub mod dock_dot;
pub mod elevation;
pub mod face;
#[cfg(target_os = "linux")]
pub mod flatpak;
pub mod folder_preview;
pub mod foot_bar;
pub mod hover_card;
pub mod icon_picker;
pub mod import;
pub mod launch;
pub mod launch_picture;
pub mod lazy_window;
pub mod link_preview;
pub mod link_rules;
pub mod link_target;
pub mod little_launch;
pub mod little_sites;
pub mod logging;
pub mod menu;
pub mod motion;
pub mod needs_you;
pub mod notes;
pub mod offer;
pub mod overlay_life;
pub mod palette;
pub mod passwords;
pub mod safe_start;
pub mod settings_card;
pub mod settings_column;
pub mod settings_pictures;
pub mod settings_rows;
pub mod settings_search;
pub mod settling;
pub mod shell;
pub mod site_controls;
pub mod space_icon;
pub mod text_input;
pub mod theme;
pub mod theme_picker;
pub mod toast;
pub mod tokens;
pub mod tooltip;
pub mod updates;
pub mod usage;
pub mod widevine;
pub mod window_frames;
pub mod zoom_hud;
