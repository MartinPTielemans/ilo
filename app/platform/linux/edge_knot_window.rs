//! The edge knot of platform/host.h on Linux: the tab the hidden sidebar leaves at its browser
//! window's edge while a thread needs the user, over the page without the keyboard, as its window
//! does on the Mac. GPUI shows it as an overlay of that window (overlays.rs).

use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
};

use gpui::{Bounds, Pixels, point, px, size};

thread_local! {
    /// Each browser window's UI view, its knot's view, and while it shows, where in the content.
    static KNOTS: RefCell<Vec<(*mut c_void, *mut c_void, Option<Bounds<Pixels>>)>> =
        const { RefCell::new(Vec::new()) };
    /// The room round the tab in its window, for its shadow, on every side but the window's edge.
    static MARGIN: Cell<f32> = const { Cell::new(0.) };
}

fn ignore_left(_: *mut c_void) {}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_edge_knot(
    ui_view: *mut c_void,
    edge_knot_view: *mut c_void,
    margin: f64,
) {
    MARGIN.set(margin as f32);
    KNOTS.with_borrow_mut(|all| {
        all.retain(|&(each, ..)| each != ui_view);
        all.push((ui_view, edge_knot_view, None));
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn host_show_edge_knot(ui_view: *mut c_void, top: f64, width: f64, height: f64) {
    let Some(content) = gpui_linux::embedder_size(ui_view) else {
        return;
    };
    let (margin, width, height) = (MARGIN.get(), width as f32, height as f32);
    let right = crate::reveal_window::sidebar_right(ui_view);
    let x = match right {
        true => content.width - px(width + margin),
        false => px(0.),
    };
    let bounds = Bounds::new(
        point(x, px(top as f32 - margin)),
        size(px(width + margin), px(height + 2. * margin)),
    );
    let Some((view, was)) = KNOTS.with_borrow_mut(|all| {
        let knot = all.iter_mut().find(|(each, ..)| *each == ui_view)?;
        Some((knot.1, knot.2.replace(bounds)))
    }) else {
        return;
    };
    // Only the tab takes the pointer; the margin round it lets it through to the page. Where its
    // window can't blend with the page, it's cut to the tab, without the shadow, as the mini bar is.
    let left = if right { margin } else { 0. };
    let tab = Bounds::new(point(px(left), px(margin)), size(px(width), px(height)));
    gpui_linux::set_overlay_pointer(view, Some(&[tab]));
    if !gpui_linux::overlays_blend() {
        let radius = browser::tokens::shell::edge_knot::RADIUS;
        let (near, far) = (
            if right { 0. } else { radius },
            if right { radius } else { 0. },
        );
        let (right_x, bottom) = (left + width, margin + height);
        let corners = [
            (left, margin, far),
            (right_x, margin, near),
            (right_x, bottom, near),
            (left, bottom, far),
        ];
        gpui_linux::set_overlay_outline(view, &corners);
    }
    match was {
        Some(_) => gpui_linux::move_overlay(view, bounds, px(0.)),
        None => crate::overlays::show_over(view, ui_view, bounds, px(0.), false, ignore_left),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_edge_knot(ui_view: *mut c_void) {
    let view = KNOTS.with_borrow_mut(|all| {
        let knot = all.iter_mut().find(|(each, ..)| *each == ui_view)?;
        knot.2.take().map(|_| knot.1)
    });
    if let Some(view) = view {
        gpui_linux::hide_overlay(view, || {});
    }
}

/// The knots' views that show, for the test driver's layout.
pub(crate) fn shown() -> Vec<*mut c_void> {
    KNOTS.with_borrow(|all| {
        (all.iter())
            .filter(|(.., at)| at.is_some())
            .map(|&(_, view, _)| view)
            .collect()
    })
}

/// The browser window's UI view a knot's `view` shows over, and where in its content, while it
/// shows, for the Wayland driver's layout.
pub(crate) fn placed(view: *mut c_void) -> Option<(*mut c_void, Bounds<Pixels>)> {
    KNOTS.with_borrow(|all| {
        (all.iter())
            .find(|&&(_, each, _)| each == view)
            .and_then(|&(ui_view, _, at)| Some((ui_view, at?)))
    })
}

/// `ui_view`'s window is closing: its knot goes.
pub(crate) fn forget_window(ui_view: *mut c_void) {
    host_hide_edge_knot(ui_view);
    KNOTS.with_borrow_mut(|all| all.retain(|&(each, ..)| each != ui_view));
}
