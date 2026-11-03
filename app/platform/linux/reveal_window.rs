//! The sidebar reveal of platform/host.h on Linux: the panel the hidden sidebar slides out in,
//! over its browser window's left edge without the keyboard, as its window does on the Mac. GPUI
//! shows it as an overlay of that window (overlays.rs).
//!
//! shortcut: the pointer over the page is Chromium's, so the host doesn't hear it move there. The
//! reveal's own window says where the pointer is over it, and that it left (src/reveal.rs).

use std::{cell::RefCell, ffi::c_void};

use gpui::{Bounds, point, px, size};

thread_local! {
    /// Each browser window's UI view, its reveal's view, and whether that shows.
    static REVEALS: RefCell<Vec<(*mut c_void, *mut c_void, bool)>> =
        const { RefCell::new(Vec::new()) };
}

fn ignore_left(_: *mut c_void) {}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_reveal(
    ui_view: *mut c_void,
    reveal_view: *mut c_void,
    _pointer_moved: extern "C" fn(ui_view: *mut c_void, x: f64),
) {
    REVEALS.with_borrow_mut(|all| {
        all.retain(|&(each, ..)| each != ui_view);
        all.push((ui_view, reveal_view, false));
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn host_show_reveal(ui_view: *mut c_void, width: f64) {
    let Some((view, was_shown)) = REVEALS.with_borrow_mut(|all| {
        let reveal = all.iter_mut().find(|(each, ..)| *each == ui_view)?;
        Some((reveal.1, std::mem::replace(&mut reveal.2, true)))
    }) else {
        return;
    };
    let Some(content) = gpui_linux::embedder_size(ui_view) else {
        return;
    };
    let bounds = Bounds::new(
        point(px(0.), px(0.)),
        size(px(width as f32), content.height),
    );
    gpui_linux::show_overlay(view, ui_view, bounds, px(0.), false, ignore_left);
    if !was_shown {
        // check.sh reads it, as the Mac's host says it.
        eprintln!("browser: reveal window shown");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_reveal(ui_view: *mut c_void) {
    let view = REVEALS.with_borrow_mut(|all| {
        let reveal = all.iter_mut().find(|(each, ..)| *each == ui_view)?;
        std::mem::replace(&mut reveal.2, false).then_some(reveal.1)
    });
    if let Some(view) = view {
        gpui_linux::hide_overlay(view, || {});
    }
}

/// The reveals' views that show, for the test driver's layout.
pub(crate) fn shown() -> Vec<*mut c_void> {
    REVEALS.with_borrow(|all| {
        (all.iter())
            .filter(|&&(.., shown)| shown)
            .map(|&(_, view, _)| view)
            .collect()
    })
}

/// `ui_view`'s window's content changed size: its reveal stays as tall.
pub(crate) fn place(ui_view: *mut c_void) {
    let view = REVEALS.with_borrow(|all| {
        (all.iter())
            .find(|&&(each, _, shown)| each == ui_view && shown)
            .map(|&(_, view, _)| view)
    });
    if let Some(view) = view
        && let (Some(content), Some(own)) = (
            gpui_linux::embedder_size(ui_view),
            gpui_linux::overlay_size(view),
        )
    {
        let bounds = Bounds::new(point(px(0.), px(0.)), size(own.width, content.height));
        gpui_linux::move_overlay(view, bounds, px(0.));
    }
}

/// `ui_view`'s window is closing: its reveal goes.
pub(crate) fn forget_window(ui_view: *mut c_void) {
    host_hide_reveal(ui_view);
    REVEALS.with_borrow_mut(|all| all.retain(|&(each, ..)| each != ui_view));
}
