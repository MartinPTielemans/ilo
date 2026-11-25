//! First run's window of platform/host.h on Linux: GPUI's window, which it opened unshown, shown as
//! a window of its own (gpui_linux::show_window). Wayland lets no app place a window, so where it
//! goes is the compositor's; there's no launch window to take down.

use std::{cell::Cell, ffi::c_void};

thread_local! {
    static FIRST_RUN: Cell<*mut c_void> = const { Cell::new(std::ptr::null_mut()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_first_run(first_run_view: *mut c_void) {
    FIRST_RUN.set(first_run_view);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_show_first_run() {
    let view = FIRST_RUN.get();
    if view.is_null() {
        browser::log!("browser: first run has no window to show");
        return;
    }
    gpui_linux::show_window(view, std::ptr::null_mut());
    // check.sh waits for this line.
    browser::log!("browser: first run shown");
}

#[unsafe(no_mangle)]
pub extern "C" fn host_close_first_run() {
    let view = FIRST_RUN.replace(std::ptr::null_mut());
    if !view.is_null() {
        gpui_linux::hide_window(view);
    }
}
