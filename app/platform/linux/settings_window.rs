//! The Settings window of platform/host.h on Linux: GPUI's window, which it opened unshown, shows as
//! a window of its own (gpui_linux::show_window), and hides keeping its place on X11.

use std::{cell::Cell, ffi::c_void};

thread_local! {
    static SETTINGS: Cell<*mut c_void> = const { Cell::new(std::ptr::null_mut()) };
    static SHOWN: Cell<bool> = const { Cell::new(false) };
    static PLACED: Cell<bool> = const { Cell::new(false) };
    /// Shown since the Wayland driver last waited for sway to focus it (drive_wayland.rs).
    static SHOWN_ANEW: Cell<bool> = const { Cell::new(false) };
}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_settings(settings_view: *mut c_void) {
    SETTINGS.set(settings_view);
}

/// Shows Settings in front with the keyboard. The first time it's centred over the browser window
/// last key, where the display lets the app place a window.
#[unsafe(no_mangle)]
pub extern "C" fn host_show_settings() {
    let view = SETTINGS.get();
    if view.is_null() {
        return;
    }
    let over = match PLACED.replace(true) {
        true => std::ptr::null_mut(),
        false => crate::front().1,
    };
    gpui_linux::show_window(view, over);
    SHOWN.set(true);
    SHOWN_ANEW.set(true);
    // check.sh waits for this line. The window's title is its tab's name, as on the Mac.
    eprintln!("browser: settings shown {}", crate::settings::title());
}

/// Settings' view while it shows, for the driver.
pub(crate) fn shown() -> Option<*mut c_void> {
    let view = SETTINGS.get();
    (SHOWN.get() && !view.is_null()).then_some(view)
}

/// Whether Settings has shown since this was last asked, for the Wayland driver.
pub(crate) fn take_shown_anew() -> bool {
    SHOWN_ANEW.replace(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_settings() {
    if SHOWN.replace(false) {
        gpui_linux::hide_window(SETTINGS.get());
    }
}
