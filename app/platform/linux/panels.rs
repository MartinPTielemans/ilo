//! The panels of platform/host.h on Linux: the find bar and the offer to save a password over the
//! top right of each browser window's page, which stay while the user is elsewhere, as their
//! windows do on the Mac. GPUI
//! shows it as a panel of the browser window (`gpui_linux::set_overlay_panel`): on X11 a window
//! inside it. It has the keyboard while the user gives it them, from the shortcut that opens it or
//! a press in it, until a press elsewhere; the core lends it the window's (overlays.rs).

use std::{cell::RefCell, ffi::c_void};

use gpui::{Bounds, point, px, size};

use crate::overlays;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    FindBar,
    Offer,
}

struct Panel {
    kind: Kind,
    /// The UI's view of the browser window it's over, and the core's id of that window.
    ui_view: *mut c_void,
    window: crate::bc::bc_id,
    view: *mut c_void,
    radius: f64,
    /// From the top right corner of the page.
    right: f64,
    top: f64,
    shown: bool,
    keyboard: bool,
}

thread_local! {
    static PANELS: RefCell<Vec<Panel>> = const { RefCell::new(Vec::new()) };
    /// Each browser window's page's left edge and top right corner, from its content's
    /// (host_set_pane_corner).
    static CORNERS: RefCell<Vec<(*mut c_void, f64, f64, f64)>> = const { RefCell::new(Vec::new()) };
}

fn with_panel<R>(matches: impl Fn(&Panel) -> bool, f: impl FnOnce(&mut Panel) -> R) -> Option<R> {
    PANELS.with_borrow_mut(|panels| panels.iter_mut().find(|panel| matches(panel)).map(f))
}

fn add(kind: Kind, ui_view: *mut c_void, view: *mut c_void, radius: f64, right: f64, top: f64) {
    gpui_linux::set_overlay_panel(view, pressed);
    PANELS.with_borrow_mut(|panels| {
        panels.retain(|panel| !(panel.ui_view == ui_view && panel.kind == kind));
        panels.push(Panel {
            kind,
            ui_view,
            window: crate::window_with_view(ui_view),
            view,
            radius,
            right,
            top,
            shown: false,
            keyboard: false,
        });
    });
}

/// Where the panel of `view` goes in its browser window's content, at its own size.
fn bounds(view: *mut c_void) -> Option<Bounds<gpui::Pixels>> {
    let (ui_view, right, top) = with_panel(|p| p.view == view, |p| (p.ui_view, p.right, p.top))?;
    let content = gpui_linux::embedder_size(ui_view)?;
    let own = gpui_linux::overlay_size(view)?;
    let (page_right, page_top) = page_corner(ui_view);
    let left = f64::from(content.width) - page_right - right - f64::from(own.width);
    // The offer goes 8 points under the find bar while it shows, which it would cover.
    let top = match with_panel(|p| p.view == view, |p| p.kind) {
        Some(Kind::Offer) => below_find_bar(ui_view).unwrap_or(top),
        _ => top,
    };
    Some(Bounds::new(
        point(px(left as f32), px((page_top + top) as f32)),
        size(own.width, own.height),
    ))
}

/// The browser window the panel of `view` shows over, by its UI's view, and where in its content,
/// if it shows, for ilo's menu over it (overlays::placed).
pub(crate) fn placed(view: *mut c_void) -> Option<(*mut c_void, Bounds<gpui::Pixels>)> {
    let ui_view = with_panel(|p| p.view == view && p.shown, |p| p.ui_view)?;
    Some((ui_view, bounds(view)?))
}

/// 8 points under `ui_view`'s find bar while it shows, from the page's top.
pub(crate) fn below_find_bar(ui_view: *mut c_void) -> Option<f64> {
    let (view, top) = with_panel(
        |p| p.ui_view == ui_view && p.kind == Kind::FindBar && p.shown,
        |p| (p.view, p.top),
    )?;
    let own = gpui_linux::overlay_size(view)?;
    Some(top + f64::from(own.height) + 8.)
}

/// The top right corner of `ui_view`'s window's page, from its content's.
pub(crate) fn page_corner(ui_view: *mut c_void) -> (f64, f64) {
    CORNERS.with_borrow(|corners| {
        (corners.iter().find(|(each, ..)| *each == ui_view))
            .map_or((0., 0.), |&(_, _, right, top)| (right, top))
    })
}

/// Where `ui_view`'s window's page starts, from its content's left, where the toast's window does.
pub(crate) fn page_left(ui_view: *mut c_void) -> f64 {
    CORNERS.with_borrow(|corners| {
        (corners.iter().find(|(each, ..)| *each == ui_view)).map_or(0., |&(_, left, ..)| left)
    })
}

fn show(kind: Kind, ui_view: *mut c_void, keyboard: bool) {
    let Some((view, window, shown, had)) = with_panel(
        |p| p.ui_view == ui_view && p.kind == kind,
        |p| (p.view, p.window, p.shown, p.keyboard),
    ) else {
        return;
    };
    let keyboard = had || keyboard;
    if keyboard && !had {
        overlays::lend_keyboard(window, view, true);
        // check.sh reads this line.
        browser::log!("browser: {} takes the keyboard", name(kind));
    }
    with_panel(
        |p| p.view == view,
        |p| {
            p.shown = true;
            p.keyboard = keyboard;
        },
    );
    if shown {
        if keyboard && !had {
            gpui_linux::set_overlay_keyboard(view, true, left);
        }
        return;
    }
    let (Some(bounds), Some(radius)) = (bounds(view), with_panel(|p| p.view == view, |p| p.radius))
    else {
        return;
    };
    crate::overlays::show_over(view, ui_view, bounds, px(radius as f32), keyboard, left);
    // check.sh reads these lines.
    let at = [
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        bounds.size.height,
    ];
    let [x, y, width, height] = at.map(f32::from);
    browser::log!("browser: {} at {x} {y} {width} {height}", name(kind));
    browser::log!("browser: {} shown", name(kind));
    if kind == Kind::FindBar {
        place(ui_view);
    }
}

fn hide(kind: Kind, ui_view: *mut c_void) {
    let Some((view, window, keyboard)) = with_panel(
        |p| p.ui_view == ui_view && p.kind == kind && p.shown,
        |p| {
            p.shown = false;
            (p.view, p.window, std::mem::take(&mut p.keyboard))
        },
    ) else {
        return;
    };
    gpui_linux::hide_overlay(view, move || {
        if keyboard {
            overlays::lend_keyboard(window, view, false);
        }
    });
    // check.sh reads this line.
    browser::log!("browser: {} hidden", name(kind));
    if kind == Kind::FindBar {
        place(ui_view);
    }
}

/// The views of the panels that show, for the test driver (drive.rs).
pub(crate) fn shown() -> Vec<*mut c_void> {
    PANELS.with_borrow(|panels| {
        (panels.iter())
            .filter(|panel| panel.shown)
            .map(|panel| panel.view)
            .collect()
    })
}

fn name(kind: Kind) -> &'static str {
    match kind {
        Kind::FindBar => "find bar",
        Kind::Offer => "offer",
    }
}

/// The user left the panel of `view` with the keyboard: a press elsewhere, or the keyboard going
/// to another window. It keeps showing, and gives the keyboard back, unless another of the app's
/// overlays took it, which gives it back to the panel as it closes.
fn left(view: *mut c_void) {
    let Some((window, keyboard)) =
        with_panel(|p| p.view == view && p.shown, |p| (p.window, p.keyboard))
    else {
        return;
    };
    if !keyboard {
        return;
    }
    if overlays::keyboard_lent_to_another(window, view) {
        gpui_linux::set_overlay_keyboard(view, true, left);
        return;
    }
    let kind = with_panel(
        |p| p.view == view,
        |p| {
            p.keyboard = false;
            p.kind
        },
    );
    gpui_linux::set_overlay_keyboard(view, false, left);
    overlays::lend_keyboard(window, view, false);
    if let Some(kind) = kind {
        // check.sh reads this line.
        browser::log!("browser: {} gives the keyboard back", name(kind));
    }
}

/// A press in the panel of `view` while it hasn't the keyboard, which gives it them, as a click
/// makes its window key on the Mac.
fn pressed(view: *mut c_void) {
    let Some((ui_view, kind)) = with_panel(|p| p.view == view && p.shown, |p| (p.ui_view, p.kind))
    else {
        return;
    };
    show(kind, ui_view, true);
}

/// Places the panels and floats that show over the browser window of `ui_view` again, as its
/// content changed size, its page moved or its find bar showed or hid.
pub(crate) fn place(ui_view: *mut c_void) {
    crate::floats::place(ui_view);
    let shown: Vec<_> = PANELS.with_borrow(|panels| {
        (panels.iter())
            .filter(|p| p.ui_view == ui_view && p.shown)
            .map(|p| (p.view, p.radius))
            .collect()
    });
    for (view, radius) in shown {
        if let Some(bounds) = bounds(view) {
            gpui_linux::move_overlay(view, bounds, px(radius as f32));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_find_bar(ui_view: *mut c_void, view: *mut c_void, right: f64, top: f64) {
    let radius = browser::tokens::find_bar::RADIUS.into();
    add(Kind::FindBar, ui_view, view, radius, right, top);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_show_find_bar(ui_view: *mut c_void, keyboard: bool) {
    show(Kind::FindBar, ui_view, keyboard);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_find_bar(ui_view: *mut c_void) {
    hide(Kind::FindBar, ui_view);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_offer(
    ui_view: *mut c_void,
    offer_view: *mut c_void,
    right: f64,
    top: f64,
) {
    let radius = browser::tokens::offer::RADIUS.into();
    add(Kind::Offer, ui_view, offer_view, radius, right, top);
}

/// Without the keyboard: as the find bar, it takes it when pressed.
#[unsafe(no_mangle)]
pub extern "C" fn host_show_offer(ui_view: *mut c_void) {
    show(Kind::Offer, ui_view, false);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_offer(ui_view: *mut c_void) {
    hide(Kind::Offer, ui_view);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_set_pane_corner(ui_view: *mut c_void, left: f64, right: f64, top: f64) {
    CORNERS.with_borrow_mut(|corners| {
        corners.retain(|(each, ..)| *each != ui_view);
        corners.push((ui_view, left, right, top));
    });
    place(ui_view);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_forget_window(ui_view: *mut c_void) {
    let kinds: Vec<Kind> = PANELS.with_borrow(|panels| {
        (panels.iter())
            .filter(|p| p.ui_view == ui_view)
            .map(|p| p.kind)
            .collect()
    });
    for kind in kinds {
        hide(kind, ui_view);
    }
    PANELS.with_borrow_mut(|panels| panels.retain(|p| p.ui_view != ui_view));
    overlays::forget_window(ui_view);
    crate::menu_window::forget_window(ui_view);
    crate::floats::forget_window(ui_view);
    crate::frames::forget_window(ui_view);
    crate::popovers::forget_window(ui_view);
    crate::active_window::forget_window(ui_view);
    crate::link_preview_window::forget_window(ui_view);
    crate::reveal_window::forget_window(ui_view);
    CORNERS.with_borrow_mut(|corners| corners.retain(|(each, ..)| *each != ui_view));
    unsafe { crate::platform::host_window_closed(ui_view) };
}
