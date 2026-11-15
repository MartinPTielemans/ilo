//! The sidebar reveal of platform/host.h on Linux: the panel the hidden sidebar slides out in,
//! over its browser window's edge on the sidebar's side without the keyboard, as its window does on
//! the Mac. GPUI shows it as an overlay of that window (overlays.rs).
//!
//! shortcut: the pointer over the page is Chromium's, so the host doesn't hear it move there. The
//! reveal's own window says where the pointer is over it, and that it left (src/reveal.rs).

use std::{cell::RefCell, ffi::c_void};

use gpui::{Bounds, Pixels, point, px, size};

thread_local! {
    /// Each browser window's UI view, its reveal's view, and whether that shows.
    static REVEALS: RefCell<Vec<(*mut c_void, *mut c_void, bool)>> =
        const { RefCell::new(Vec::new()) };
    /// The UI views of the browser windows whose sidebar is on the right.
    static RIGHT: RefCell<Vec<*mut c_void>> = const { RefCell::new(Vec::new()) };
}

/// The window's buttons are the UI's own here, which it draws on the sidebar's side, so only the
/// reveal moves: it comes out over the window's right edge.
#[unsafe(no_mangle)]
pub extern "C" fn host_set_sidebar_right(ui_view: *mut c_void, right: bool) {
    RIGHT.with_borrow_mut(|all| {
        all.retain(|&each| each != ui_view);
        if right {
            all.push(ui_view);
        }
    });
    place(ui_view);
}

/// Where a reveal `width` wide goes in `ui_view`'s window's content `content` wide: at its left
/// edge, or at its right with the sidebar there.
fn reveal_x(ui_view: *mut c_void, content: Pixels, width: Pixels) -> Pixels {
    match RIGHT.with_borrow(|all| all.contains(&ui_view)) {
        true => content - width,
        false => px(0.),
    }
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
    let width = px(width as f32);
    let bounds = Bounds::new(
        point(reveal_x(ui_view, content.width, width), px(0.)),
        size(width, content.height),
    );
    crate::overlays::show_over(view, ui_view, bounds, px(0.), false, ignore_left);
    if !was_shown {
        // check.sh reads it, as the Mac's host says it.
        browser::log!("browser: reveal window shown");
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

/// The browser window's UI view a reveal's `view` shows over, and where in its content, while it
/// shows, for the Wayland driver's layout, as overlays.rs's own aren't the reveal.
pub(crate) fn placed(view: *mut c_void) -> Option<(*mut c_void, Bounds<Pixels>)> {
    let ui_view = REVEALS.with_borrow(|all| {
        (all.iter())
            .find(|&&(_, each, shown)| each == view && shown)
            .map(|&(ui_view, ..)| ui_view)
    })?;
    let own = gpui_linux::overlay_size(view)?;
    let content = gpui_linux::embedder_size(ui_view)?;
    let x = reveal_x(ui_view, content.width, own.width);
    Some((ui_view, Bounds::new(point(x, px(0.)), own)))
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
        let x = reveal_x(ui_view, content.width, own.width);
        let bounds = Bounds::new(point(x, px(0.)), size(own.width, content.height));
        gpui_linux::move_overlay(view, bounds, px(0.));
    }
}

/// `ui_view`'s window is closing: its reveal goes.
pub(crate) fn forget_window(ui_view: *mut c_void) {
    host_hide_reveal(ui_view);
    REVEALS.with_borrow_mut(|all| all.retain(|&(each, ..)| each != ui_view));
    RIGHT.with_borrow_mut(|all| all.retain(|&each| each != ui_view));
}
