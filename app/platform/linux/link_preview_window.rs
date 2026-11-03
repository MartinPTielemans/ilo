//! The link preview of platform/host.h on Linux. Its window, the dim and the card, is an overlay
//! over the browser window (overlays.rs). No window of the app's can hold a page here, so the page
//! shows in the browser window, over its own pages (the core's ShowPageIn), and the overlay has a
//! hole there, where it draws its card round it. The page has the window's keyboard, as a tab's
//! would, and the core hands the preview the keys it acts on, as on the Mac.
//!
//! shortcut: the page doesn't scale with the card's motion, as the Mac's does. It's laid out where
//! it rests, and shows where the frame the card draws meets that.
//!
//! Another browser window or Settings taking the keyboard is the user leaving the preview, as
//! another window becoming key is on the Mac. Switching apps isn't: the preview stays.

use std::{cell::RefCell, ffi::c_void};

use gpui::{Bounds, Pixels, Size, point, px, size};

use crate::bc;

struct Preview {
    view: *mut c_void,
    dismissed: Option<extern "C" fn()>,
    shown: Option<Shown>,
}

#[derive(Clone, Copy)]
struct Shown {
    /// The UI's view of the browser window it covers, and the core's id of that.
    ui_view: *mut c_void,
    window: bc::bc_id,
    content: Size<Pixels>,
    /// Where the page shows, and its corners, from the frame the UI drew last.
    hole: Option<(Bounds<Pixels>, f32)>,
    /// It takes no more clicks as it leaves (host_release_link_preview).
    released: bool,
}

thread_local! {
    static PREVIEW: RefCell<Preview> = const {
        RefCell::new(Preview { view: std::ptr::null_mut(), dismissed: None, shown: None })
    };
}

/// The views of the preview's window while it shows, for the test driver (drive.rs).
pub(crate) fn shown() -> Vec<*mut c_void> {
    PREVIEW.with_borrow(|preview| preview.shown.map(|_| preview.view).into_iter().collect())
}

/// The overlay's outline: all of the window's content, with a hole for the page through a cut down
/// from the top edge, which the X server's shape takes as one polygon.
fn outline(content: Size<Pixels>, hole: Option<(Bounds<Pixels>, f32)>) -> Vec<gpui_linux::Corner> {
    let (width, height) = (f32::from(content.width), f32::from(content.height));
    let mut corners = vec![(0., 0., 0.)];
    // The cut leaves the top edge and comes back to it, between its corners.
    if let Some((hole, radius)) = hole {
        let (left, top) = (f32::from(hole.left()), f32::from(hole.top()));
        let (right, bottom) = (f32::from(hole.right()), f32::from(hole.bottom()));
        let middle = (left + right) / 2.;
        corners.extend([
            (middle, 0., 0.),
            (middle, top, 0.),
            (left, top, radius),
            (left, bottom, radius),
            (right, bottom, radius),
            (right, top, radius),
            (middle, top, 0.),
            (middle, 0., 0.),
        ]);
    }
    corners.extend([(width, 0., 0.), (width, height, 0.), (0., height, 0.)]);
    corners
}

/// Where the overlay takes the pointer: all but the hole, whose clicks are the page's.
fn around(content: Size<Pixels>, hole: Bounds<Pixels>) -> [Bounds<Pixels>; 4] {
    let (width, height) = (content.width, content.height);
    let rows = hole.size.height;
    [
        Bounds::new(point(px(0.), px(0.)), size(width, hole.top())),
        Bounds::new(point(px(0.), hole.bottom()), size(width, height - hole.bottom())),
        Bounds::new(point(px(0.), hole.top()), size(hole.left(), rows)),
        Bounds::new(point(hole.right(), hole.top()), size(width - hole.right(), rows)),
    ]
}

/// Puts the overlay over all of its window's content, with its hole where the page shows, and has
/// it take the clicks but there, until it's released.
fn place(view: *mut c_void, shown: &Shown) {
    gpui_linux::set_overlay_outline(view, &outline(shown.content, shown.hole));
    let areas = match (shown.released, shown.hole) {
        (true, _) => Some(vec![]),
        (false, Some((hole, _))) => Some(around(shown.content, hole).to_vec()),
        (false, None) => None,
    };
    gpui_linux::set_overlay_pointer(view, areas.as_deref());
    // Which puts its outline in place too.
    gpui_linux::move_overlay(view, Bounds::new(point(px(0.), px(0.)), shown.content), px(0.));
}

/// The user left the preview: a click in its page, as GPUI hears the browser window's too, or
/// another window or app. It stays, as its page has the keyboard.
fn left(_: *mut c_void) {}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_link_preview(
    view: *mut c_void,
    dismissed: extern "C" fn(),
) -> *mut c_void {
    PREVIEW.with_borrow_mut(|preview| {
        preview.view = view;
        preview.dismissed = Some(dismissed);
    });
    // The core shows the page in the browser window, under this one, and only needs to know there's
    // a view it goes with.
    view
}

#[unsafe(no_mangle)]
pub extern "C" fn host_show_link_preview(ui_view: *mut c_void) {
    if PREVIEW.with_borrow(|preview| preview.shown.is_some_and(|shown| shown.ui_view != ui_view)) {
        host_hide_link_preview();
    }
    let Some(content) = gpui_linux::embedder_size(ui_view) else {
        return;
    };
    let window = crate::window_with_view(ui_view);
    // All of it until its first frame says where the page is.
    let shown = Shown { ui_view, window, content, hole: None, released: false };
    let view = PREVIEW.with_borrow_mut(|preview| {
        preview.shown = Some(shown);
        preview.view
    });
    if view.is_null() {
        return;
    }
    gpui_linux::set_overlay_outline(view, &outline(content, None));
    gpui_linux::set_overlay_pointer(view, None);
    let bounds = Bounds::new(point(px(0.), px(0.)), content);
    gpui_linux::show_overlay(view, ui_view, bounds, px(0.), false, left);
    // check.sh reads this line.
    eprintln!("browser: link preview shown");
}

#[unsafe(no_mangle)]
pub extern "C" fn host_link_preview_drew(
    left: f64,
    top: f64,
    width: f64,
    height: f64,
    radius: f64,
    _opacity: f64,
) {
    let hole = (width >= 1. && height >= 1.).then(|| {
        let hole = Bounds::new(
            point(px(left as f32), px(top as f32)),
            size(px(width as f32), px(height as f32)),
        );
        (hole, (radius as f32).min(width.min(height) as f32 / 2.))
    });
    let placed = PREVIEW.with_borrow_mut(|preview| {
        let shown = preview.shown.as_mut().filter(|shown| shown.hole != hole)?;
        shown.hole = hole;
        Some((preview.view, *shown))
    });
    if let Some((view, shown)) = placed {
        place(view, &shown);
    }
}

/// Has the preview take no more clicks, if it shows over a window other than `except`'s. Returns
/// what it shows over, if it did.
fn release(except: *mut c_void) -> Option<Shown> {
    let (view, shown) = PREVIEW.with_borrow_mut(|preview| {
        let shown = (preview.shown.as_mut())
            .filter(|shown| !shown.released && shown.ui_view != except)?;
        shown.released = true;
        Some((preview.view, *shown))
    })?;
    place(view, &shown);
    // check.sh reads this line.
    eprintln!("browser: link preview released");
    Some(shown)
}

#[unsafe(no_mangle)]
pub extern "C" fn host_release_link_preview() {
    if let Some(shown) = release(std::ptr::null_mut()) {
        // The window's page has the keyboard back from the preview's.
        unsafe { bc::bc_window_focus_page(shown.window) };
    }
}

/// The window of `ui_view`, a browser window or Settings, took the keyboard. If it isn't the one
/// the preview shows over, the user left the preview for it, and the keys stay there.
pub(crate) fn took_keyboard(ui_view: *mut c_void) {
    let dismissed = PREVIEW.with_borrow(|preview| preview.dismissed);
    if let (Some(dismissed), Some(_)) = (dismissed, release(ui_view))
        && let Some(cx) = crate::CX.with_borrow(Clone::clone)
    {
        // After the event, which the app may be handling.
        cx.foreground_executor().spawn(async move { dismissed() }).detach();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_link_preview() {
    host_release_link_preview();
    let view = PREVIEW.with_borrow_mut(|preview| preview.shown.take().map(|_| preview.view));
    if let Some(view) = view {
        gpui_linux::hide_overlay(view, || {});
        // check.sh reads this line.
        eprintln!("browser: link preview hidden");
    }
}

/// `ui_view`'s window's content may have changed size, which the preview covers all of.
pub(crate) fn resized(ui_view: *mut c_void) {
    let placed = PREVIEW.with_borrow_mut(|preview| {
        let shown = preview.shown.as_mut().filter(|shown| shown.ui_view == ui_view)?;
        shown.content = gpui_linux::embedder_size(ui_view)?;
        Some((preview.view, *shown))
    });
    if let Some((view, shown)) = placed {
        place(view, &shown);
    }
}

/// `ui_view`'s window is closing: the preview over it goes.
pub(crate) fn forget_window(ui_view: *mut c_void) {
    if PREVIEW.with_borrow(|preview| preview.shown.is_some_and(|shown| shown.ui_view == ui_view)) {
        host_hide_link_preview();
    }
}
