//! The floats of platform/host.h on Linux: the zoom HUD, the split view's drop targets, the toast,
//! the tooltip, the link target, the hover card and the foot bar, which float over a browser window
//! without the keyboard, as their windows do on the Mac. GPUI shows each as an overlay of that
//! window (overlays.rs) that lets the pointer through to what's under it, but for the toast's plate
//! while it has buttons, the hover card's, for its buttons, and the foot bar, all of it.
//!
//! The hover card's motion is the UI's (src/hover_card.rs), from its window's first frame on
//! screen, so the host shows and hides it at once. On X11 with no compositor, which draws an
//! overlay's pixels opaque, its window is cut to its plate, without the shadow round it.
//!
//! A scroll over the page doesn't end the hover card at once, as the Mac's does, and needn't: the
//! card shows only while the pointer rests on a row or on the card, and goes 150 ms after it leaves
//! them (hover_card.rs's LEAVE), so the pointer is over the page only for that long. A scroll over
//! the sidebar's rows ends it, in the UI.
//!
//! The tooltip and the link target fade in and out as the UI asks, as the Mac's do, drawn by GPUI
//! (motion.rs).
//!
//! The link target hears the pointer over the page while it shows, which Chromium's windows have
//! on Linux, through GPUI (`gpui_linux::watch_pointer_over`), so it moves out of the pointer's way.

use std::{
    cell::{Cell, RefCell},
    ffi::{c_int, c_void},
};

use gpui::{Bounds, Pixels, Point, point, px, size};

use crate::motion::{self, EASE_OUT, Motion, Track};

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Hud,
    DropTargets,
    Toast,
    Tooltip,
    LinkTarget,
    Card,
    FootBar,
}

struct Float {
    kind: Kind,
    view: *mut c_void,
    /// The UI's view of the browser window it shows over; null while hidden.
    over: *mut c_void,
    /// The one it's still fading out over, once hidden.
    fading: *mut c_void,
    /// How many times it was shown or hidden, so a fade's end that came after another knows.
    changes: u32,
    /// Where the UI asked, from the window's content's top left: the tooltip's and the link
    /// target's own corner, which their window holds `margin` in from its own.
    left: f64,
    top: f64,
}

#[derive(Default)]
struct Hud {
    right: f64,
    top: f64,
    margin: f64,
}

/// Where the foot bar goes (host_show_foot_bar).
#[derive(Default)]
struct FootBar {
    left: f64,
    right: f64,
    bottom: f64,
    width: f64,
    height: f64,
}

#[derive(Default)]
struct Toast {
    takes: bool,
    plate: Option<Bounds<Pixels>>,
}

thread_local! {
    static FLOATS: RefCell<Vec<Float>> = const { RefCell::new(Vec::new()) };
    static HUD: RefCell<Hud> = RefCell::default();
    static TOAST: RefCell<Toast> = RefCell::default();
    static FOOT_BAR: RefCell<FootBar> = RefCell::default();
    /// The tooltip's and the link target's margins, and what runs as the user leaves the tooltip's
    /// window.
    static TOOLTIP: RefCell<(f64, Option<extern "C" fn()>)> = const { RefCell::new((0., None)) };
    /// The overlay holding what the tooltip names, which it shows over, and that overlay's view.
    static TOOLTIP_OVERLAY: Cell<Option<(c_int, *mut c_void)>> = const { Cell::new(None) };
    static LINK_MARGIN: RefCell<f64> = const { RefCell::new(0.) };
    /// Where the link target tells the UI the pointer is, over the window it shows over.
    static LINK_POINTER: Cell<Option<extern "C" fn(f64, f64)>> = const { Cell::new(None) };
    /// The hover card's margin round its plate, and what runs as the user leaves it.
    static CARD: RefCell<(f64, Option<extern "C" fn()>)> = const { RefCell::new((0., None)) };
}

fn add(kind: Kind, view: *mut c_void) {
    // None of them takes the pointer until the toast has buttons.
    gpui_linux::set_overlay_pointer(view, Some(&[]));
    if matches!(kind, Kind::Tooltip | Kind::LinkTarget) {
        motion::attach(view);
    }
    FLOATS.with_borrow_mut(|floats| {
        floats.retain(|float| float.kind != kind);
        floats.push(Float {
            kind,
            view,
            over: std::ptr::null_mut(),
            fading: std::ptr::null_mut(),
            changes: 0,
            left: 0.,
            top: 0.,
        });
    });
}

/// The views of the floats that show, for the test driver (drive.rs). The X server finds the toast
/// under the pointer only on its plate while it has buttons, and the others not at all.
pub(crate) fn shown() -> Vec<*mut c_void> {
    FLOATS.with_borrow(|floats| {
        (floats.iter())
            .filter(|float| !float.over.is_null())
            .map(|float| float.view)
            .collect()
    })
}

/// The browser window the float of `view` shows over, by its UI's view, and where in its content,
/// for the Wayland driver's layout, as overlays::placed says it for the overlays.
pub(crate) fn placed(view: *mut c_void) -> Option<(*mut c_void, Bounds<Pixels>)> {
    let (kind, over) = FLOATS.with_borrow(|floats| {
        (floats.iter())
            .find(|float| float.view == view && !float.over.is_null())
            .map(|float| (float.kind, float.over))
    })?;
    Some((over, bounds(kind, over)?))
}

fn find<R>(kind: Kind, f: impl FnOnce(&mut Float) -> R) -> Option<R> {
    FLOATS.with_borrow_mut(|floats| floats.iter_mut().find(|float| float.kind == kind).map(f))
}

/// Where the float `kind` goes over `ui_view`'s window, at its own size but for the drop targets,
/// which cover the window's content, and the toast, as wide as the page.
fn bounds(kind: Kind, ui_view: *mut c_void) -> Option<Bounds<Pixels>> {
    let (view, left, top) = find(kind, |float| (float.view, float.left, float.top))?;
    let content = gpui_linux::embedder_size(ui_view)?;
    let own = gpui_linux::overlay_size(view)?;
    let width = f64::from(content.width);
    let (x, y) = match kind {
        Kind::DropTargets => return Some(Bounds::new(point(px(0.), px(0.)), content)),
        // At the focused page's corner, which in a split view is its pane's, and as wide as that
        // page from its left edge, so no toast runs past its window (#1462).
        Kind::Toast => {
            let (page_right, page_top) = crate::panels::page_corner(ui_view);
            let left = crate::panels::page_left(ui_view);
            let across = (width - page_right - left).max(0.);
            return Some(Bounds::new(
                point(px(left as f32), px(page_top as f32)),
                size(px(across as f32), own.height),
            ));
        }
        Kind::Hud => {
            let (page_right, page_top) = crate::panels::page_corner(ui_view);
            let (right, top, margin) = HUD.with_borrow(|hud| (hud.right, hud.top, hud.margin));
            // 8 points below the find bar while it shows, where the HUD would cover it.
            let top = crate::panels::below_find_bar(ui_view).map_or(top, |below| below - margin);
            (
                width - page_right - right - f64::from(own.width),
                page_top + top,
            )
        }
        Kind::Tooltip => {
            let margin = TOOLTIP.with_borrow(|(margin, _)| *margin);
            (left - margin, top - margin)
        }
        Kind::LinkTarget => {
            let margin = LINK_MARGIN.with_borrow(|margin| *margin);
            (left - margin, top - margin)
        }
        Kind::Card => {
            let margin = CARD.with_borrow(|(margin, _)| *margin);
            (left - margin, top - margin)
        }
        // Centred between its insets, its bottom over the content's, at the size the UI gave.
        Kind::FootBar => {
            let bar = FOOT_BAR
                .with_borrow(|bar| (bar.left, bar.right, bar.bottom, bar.width, bar.height));
            let (left, right, bottom, bar_width, bar_height) = bar;
            let x = left + (width - left - right - bar_width) / 2.;
            let y = f64::from(content.height) - bottom - bar_height;
            return Some(Bounds::new(
                point(px(x as f32), px(y as f32)),
                size(px(bar_width as f32), px(bar_height as f32)),
            ));
        }
    };
    Some(Bounds::new(
        point(px(x as f32), px(y as f32)),
        size(own.width, own.height),
    ))
}

/// Shows the float `kind` over `ui_view`'s window, or moves it there, and says where for check.sh.
fn show(kind: Kind, ui_view: *mut c_void) -> Option<Bounds<Pixels>> {
    let (over, fading) = find(kind, |float| (float.over, float.fading))?;
    if [over, fading]
        .iter()
        .any(|each| !each.is_null() && *each != ui_view)
    {
        hide(kind);
    }
    let bounds = bounds(kind, ui_view)?;
    let view = find(kind, |float| {
        float.over = ui_view;
        float.view
    })?;
    crate::overlays::show_over(view, ui_view, bounds, radius(kind, bounds), false, left);
    Some(bounds)
}

/// The float's corners, which X11 with no compositor cuts its window to: the foot bar's, one line's
/// or two lines' by its height, and none for the others, which draw their own.
fn radius(kind: Kind, bounds: Bounds<Pixels>) -> Pixels {
    use browser::tokens::foot_bar::{RADIUS, TALL_HEIGHT, TALL_RADIUS};
    match kind {
        Kind::FootBar if f32::from(bounds.size.height) >= TALL_HEIGHT => px(TALL_RADIUS),
        Kind::FootBar => px(RADIUS),
        _ => px(0.),
    }
}

/// Hides the float `kind` at once, and says whether it showed: it may have been fading out.
fn hide(kind: Kind) -> bool {
    let Some((view, over, fading)) = find(kind, |float| {
        float.changes += 1;
        let over = std::mem::replace(&mut float.over, std::ptr::null_mut());
        (
            float.view,
            over,
            std::mem::replace(&mut float.fading, std::ptr::null_mut()),
        )
    }) else {
        return false;
    };
    if over.is_null() && fading.is_null() {
        return false;
    }
    if kind == Kind::LinkTarget {
        gpui_linux::watch_pointer_over(std::ptr::null_mut(), None);
    }
    gpui_linux::hide_overlay(view, || {});
    !over.is_null()
}

/// Shows the tooltip or the link target over `ui_view`'s window, fading in over `fade` seconds from
/// where it is, as one fading out over that window goes back to full, and then says it's `shown`.
fn fade_in(kind: Kind, ui_view: *mut c_void, fade: f64, shown: &'static str) {
    let Some((view, over, fading)) = find(kind, |float| (float.view, float.over, float.fading))
    else {
        return;
    };
    let from = match ui_view == over || ui_view == fading {
        true => motion::opacity(view),
        false => 0.,
    };
    let fade = seconds(fade);
    let opacity = Track {
        from,
        to: 1.,
        seconds: fade,
        curve: EASE_OUT,
    };
    motion::start(view, Motion::fade(opacity));
    if show(kind, ui_view).is_none() {
        return;
    }
    let Some(change) = find(kind, |float| {
        float.fading = std::ptr::null_mut();
        float.changes += 1;
        float.changes
    }) else {
        return;
    };
    motion::after(fade, move || {
        if find(kind, |float| float.changes) == Some(change) {
            // check.sh waits for this line, so a capture after it shows the float whole.
            browser::log!("{shown}");
        }
    });
}

/// Hides the tooltip or the link target, fading out over `fade` seconds; with none, at once, which
/// also ends a fade out.
fn fade_out(kind: Kind, fade: f64) {
    let fade = seconds(fade);
    if fade == 0. {
        hide(kind);
        return;
    }
    let Some(Some((view, change))) = find(kind, |float| {
        if float.over.is_null() {
            return None;
        }
        float.fading = std::mem::replace(&mut float.over, std::ptr::null_mut());
        float.changes += 1;
        Some((float.view, float.changes))
    }) else {
        return;
    };
    let from = motion::opacity(view);
    motion::start(
        view,
        Motion::fade(Track {
            from,
            to: 0.,
            seconds: fade,
            curve: EASE_OUT,
        }),
    );
    motion::after(fade, move || {
        if find(kind, |float| float.changes) == Some(change) {
            hide(kind);
        }
    });
}

/// A fade's seconds, none where the float can't blend with what's under it.
fn seconds(fade: f64) -> f32 {
    match gpui_linux::overlays_blend() {
        true => fade as f32,
        false => 0.,
    }
}

/// The user left the float of `view`, by a press elsewhere or the keyboard going to another
/// window: the tooltip and the hover card go, as they do on the Mac when the browser window stops
/// being key.
fn left(view: *mut c_void) {
    for (kind, away) in [
        (Kind::Tooltip, TOOLTIP.with_borrow(|(_, away)| *away)),
        (Kind::Card, CARD.with_borrow(|(_, away)| *away)),
    ] {
        let shown = find(kind, |float| float.view == view && !float.over.is_null());
        if shown == Some(true)
            && let Some(away) = away
        {
            away();
        }
    }
}

/// Places the floats over `ui_view`'s window again, as its content changed size, its page moved or
/// its find bar showed or hid.
pub(crate) fn place(ui_view: *mut c_void) {
    for kind in [Kind::Hud, Kind::DropTargets, Kind::Toast, Kind::FootBar] {
        let shown = find(kind, |float| (float.view, float.over == ui_view));
        if let Some((view, true)) = shown
            && let Some(bounds) = bounds(kind, ui_view)
        {
            gpui_linux::move_overlay(view, bounds, radius(kind, bounds));
        }
    }
}

/// `ui_view`'s window is closing: what floats over it goes.
pub(crate) fn forget_window(ui_view: *mut c_void) {
    for kind in [
        Kind::Hud,
        Kind::DropTargets,
        Kind::Toast,
        Kind::Tooltip,
        Kind::LinkTarget,
        Kind::Card,
        Kind::FootBar,
    ] {
        if find(kind, |float| {
            float.over == ui_view || float.fading == ui_view
        }) == Some(true)
        {
            hide(kind);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_foot_bar(foot_bar_view: *mut c_void) {
    add(Kind::FootBar, foot_bar_view);
    // All of it takes the pointer, for its buttons, and the keyboard stays with the browser window.
    gpui_linux::set_overlay_pointer(foot_bar_view, None);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_show_foot_bar(
    ui_view: *mut c_void,
    left: f64,
    right: f64,
    bottom: f64,
    width: f64,
    height: f64,
) {
    FOOT_BAR.set(FootBar {
        left,
        right,
        bottom,
        width,
        height,
    });
    let before = find(Kind::FootBar, |float| float.over);
    if show(Kind::FootBar, ui_view).is_some() && before != Some(ui_view) {
        eprintln!("browser: foot bar shown");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_foot_bar() {
    if hide(Kind::FootBar) {
        eprintln!("browser: foot bar hidden");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_hud(hud_view: *mut c_void, right: f64, top: f64, margin: f64) {
    HUD.set(Hud { right, top, margin });
    add(Kind::Hud, hud_view);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_show_hud(ui_view: *mut c_void) {
    if show(Kind::Hud, ui_view).is_some() {
        // check.sh reads this line.
        browser::log!("browser: zoom hud shown");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_hud() {
    if hide(Kind::Hud) {
        browser::log!("browser: zoom hud hidden");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_drop_targets(view: *mut c_void) {
    add(Kind::DropTargets, view);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_show_drop_targets(ui_view: *mut c_void) {
    if find(Kind::DropTargets, |float| float.over == ui_view) == Some(true) {
        return;
    }
    if show(Kind::DropTargets, ui_view).is_some() {
        browser::log!("browser: drop targets shown");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_drop_targets() {
    if hide(Kind::DropTargets) {
        browser::log!("browser: drop targets hidden");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_toast(toast_view: *mut c_void) {
    add(Kind::Toast, toast_view);
}

/// Where its window can't blend with the page under it (X11 with no compositor), the toast over
/// `ui_view` is cut to its `plate` once the UI says where that is, at rest, as the hover card is,
/// since its margin, as wide as the page, would draw opaque over it. As it arrives, all of its
/// window shows, as any float's does: one with no shape at all is never seen, and never arrives.
fn toast_outline(ui_view: *mut c_void, plate: Option<Bounds<Pixels>>) {
    if gpui_linux::overlays_blend() {
        return;
    }
    let view = find(Kind::Toast, |float| float.view);
    let (Some(view), Some(bounds)) = (view, bounds(Kind::Toast, ui_view)) else {
        return;
    };
    let whole = Bounds::new(point(px(0.), px(0.)), bounds.size);
    let radius = browser::tokens::toast::RADIUS;
    let (outline, radius) = plate.map_or((whole, 0.), |plate| (plate, radius));
    let (left, top) = (f32::from(outline.left()), f32::from(outline.top()));
    let (right, bottom) = (f32::from(outline.right()), f32::from(outline.bottom()));
    let corners = [(left, top), (right, top), (right, bottom), (left, bottom)];
    gpui_linux::set_overlay_outline(view, &corners.map(|(x, y)| (x, y, radius)));
    gpui_linux::move_overlay(view, bounds, px(0.));
}

/// The toast takes the pointer on its plate while it has buttons, and lets it through elsewhere.
fn toast_pointer() {
    let Some(view) = find(Kind::Toast, |float| float.view) else {
        return;
    };
    let plate = TOAST.with_borrow(|toast| toast.plate.filter(|_| toast.takes));
    gpui_linux::set_overlay_pointer(view, Some(plate.as_slice()));
}

/// The toast's view and its plate, the part of its window that takes the pointer, while it shows
/// with buttons: the driver clicks there only where a pointer would land on it (drive.rs).
pub(crate) fn toast_plate() -> Option<(*mut c_void, Bounds<Pixels>)> {
    let (view, over) = find(Kind::Toast, |float| (float.view, float.over))?;
    let plate = TOAST.with_borrow(|toast| toast.plate.filter(|_| toast.takes))?;
    (!over.is_null()).then_some((view, plate))
}

#[unsafe(no_mangle)]
pub extern "C" fn host_toast_takes_clicks(takes: bool) {
    TOAST.with_borrow_mut(|toast| toast.takes = takes);
    toast_pointer();
}

#[unsafe(no_mangle)]
pub extern "C" fn host_set_toast_plate(x: f64, y: f64, width: f64, height: f64) {
    let plate = Bounds::new(
        point(px(x as f32), px(y as f32)),
        size(px(width as f32), px(height as f32)),
    );
    TOAST.with_borrow_mut(|toast| toast.plate = Some(plate));
    toast_pointer();
    if let Some(over) = find(Kind::Toast, |float| float.over).filter(|over| !over.is_null()) {
        toast_outline(over, Some(plate));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_show_toast(ui_view: *mut c_void) {
    if find(Kind::Toast, |float| float.over == ui_view) == Some(true) {
        return;
    }
    toast_outline(ui_view, None);
    if show(Kind::Toast, ui_view).is_some() {
        // check.sh reads this line.
        browser::log!("browser: toast window shown");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_toast() {
    if hide(Kind::Toast) {
        TOAST.with_borrow_mut(|toast| toast.plate = None);
        toast_pointer();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_tooltip(tooltip_view: *mut c_void, margin: f64, away: extern "C" fn()) {
    TOOLTIP.set((margin, Some(away)));
    add(Kind::Tooltip, tooltip_view);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_show_tooltip(
    ui_view: *mut c_void,
    overlay: c_int,
    left: f64,
    top: f64,
    fade: f64,
) {
    let owner = match overlay {
        -1 => None,
        overlay => match crate::overlays::shown_at(overlay) {
            Some((view, bounds)) => Some((overlay, view, bounds)),
            None => return,
        },
    };
    let Some(view) = find(Kind::Tooltip, |float| float.view) else {
        return;
    };
    // An overlay goes over a tooltip from the browser window, as a new overlay shows over those
    // that show, but the one holding what it names goes directly under it.
    let was = TOOLTIP_OVERLAY.replace(owner.map(|(overlay, view, _)| (overlay, view)));
    if let Some((_, under)) = was.filter(|was| Some(was.0) != owner.map(|owner| owner.0)) {
        gpui_linux::set_overlay_under(under, std::ptr::null_mut());
    }
    if let Some((_, under, _)) = owner {
        gpui_linux::set_overlay_under(under, view);
    }
    let (x, y) = owner.map_or((0., 0.), |(_, _, bounds)| {
        (f64::from(bounds.origin.x), f64::from(bounds.origin.y))
    });
    find(Kind::Tooltip, |float| {
        (float.left, float.top) = (x + left, y + top)
    });
    // check.sh reads this line: where it is, in the window it shows over, to compare with where
    // the UI asked.
    browser::log!("browser: tooltip window at {left} {top}");
    fade_in(
        Kind::Tooltip,
        ui_view,
        fade,
        "browser: tooltip window shown",
    );
    // On X11 a window put over the browser window goes directly over it, under the overlays, so the
    // overlay is put under the tooltip again, and its shadow under it.
    if let Some((overlay, ..)) = owner {
        crate::overlays::restack(overlay);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_tooltip(fade: f64) {
    fade_out(Kind::Tooltip, fade);
}

/// Overlay `overlay` is hiding: the tooltip over it, if it shows, goes at once.
pub(crate) fn overlay_hiding(overlay: c_int) {
    if let Some((owner, under)) = TOOLTIP_OVERLAY.get()
        && owner == overlay
    {
        TOOLTIP_OVERLAY.set(None);
        gpui_linux::set_overlay_under(under, std::ptr::null_mut());
        if hide(Kind::Tooltip)
            && let Some(away) = TOOLTIP.with_borrow(|(_, away)| *away)
        {
            away();
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_link_target(
    link_view: *mut c_void,
    margin: f64,
    pointer: extern "C" fn(x: f64, y: f64),
) {
    LINK_MARGIN.set(margin);
    LINK_POINTER.set(Some(pointer));
    add(Kind::LinkTarget, link_view);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_show_link_target(ui_view: *mut c_void, left: f64, top: f64, fade: f64) {
    find(Kind::LinkTarget, |float| {
        (float.left, float.top) = (left, top)
    });
    // check.sh reads this line: which side it's on, out of the pointer's way.
    browser::log!("browser: link target window at {left} {top}");
    fade_in(
        Kind::LinkTarget,
        ui_view,
        fade,
        "browser: link target window shown",
    );
    // Only while it shows, so an idle browser is never woken by the pointer.
    gpui_linux::watch_pointer_over(ui_view, Some(link_pointer));
}

/// The pointer moved over the window the link target shows over: the UI hears it after the event.
fn link_pointer(at: Point<Pixels>) {
    let Some(pointer) = LINK_POINTER.get() else {
        return;
    };
    let (x, y) = (f64::from(f32::from(at.x)), f64::from(f32::from(at.y)));
    if let Some(cx) = crate::CX.with_borrow(Clone::clone) {
        cx.foreground_executor()
            .spawn(async move { pointer(x, y) })
            .detach();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_link_target(fade: f64) {
    gpui_linux::watch_pointer_over(std::ptr::null_mut(), None);
    fade_out(Kind::LinkTarget, fade);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_hover_card(card_view: *mut c_void, margin: f64, away: extern "C" fn()) {
    CARD.set((margin, Some(away)));
    add(Kind::Card, card_view);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_show_hover_card(
    ui_view: *mut c_void,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
) {
    let Some(view) = find(Kind::Card, |float| {
        (float.left, float.top) = (left, top);
        float.view
    }) else {
        return;
    };
    // Only its plate takes the pointer, for its buttons; the margin round it lets it through.
    let margin = CARD.with_borrow(|(margin, _)| *margin) as f32;
    let (width, height) = (width as f32, height as f32);
    let plate = Bounds::new(point(px(margin), px(margin)), size(px(width), px(height)));
    gpui_linux::set_overlay_pointer(view, Some(&[plate]));
    if !gpui_linux::overlays_blend() {
        let radius = browser::tokens::hover_card::RADIUS;
        let (right, bottom) = (margin + width, margin + height);
        let corners = [
            (margin, margin),
            (right, margin),
            (right, bottom),
            (margin, bottom),
        ];
        gpui_linux::set_overlay_outline(view, &corners.map(|(x, y)| (x, y, radius)));
    }
    if show(Kind::Card, ui_view).is_some() {
        // check.sh reads these: where it is, to compare with where the UI asked, and that it shows.
        browser::log!("browser: hover card window at {left} {top}");
        browser::log!("browser: hover card window shown");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_hover_card() {
    hide(Kind::Card);
}
