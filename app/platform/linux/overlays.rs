//! The overlays of platform/host.h on Linux: GPUI's Linux platform shows each over the browser
//! window: a subsurface of it under Wayland, and on X11 a window of its own over it, as the X
//! server blends no child window's edges and a compositor blends a window's. The core lends it the
//! window's keyboard while it shows.
//!
//! Each casts its shadow from a window under it (shadows.rs). The icon and theme pickers grow in
//! from their arrows, the theme picker shrinks back to it, and the folder preview grows a little
//! and fades, as on the Mac, drawn by GPUI (motion.rs).
//!
//! The icon and theme pickers blur what's behind them at rest, as on the Mac, where the compositor
//! can (gpui_linux::overlays_blur), and their card is translucent there (palette.rs, card_drawn).
//! The command bar's and the folder preview's fill is opaque on every OS, so no blur would show.

use std::{
    cell::{Cell, RefCell},
    ffi::{c_int, c_void},
    time::Instant,
};

use gpui::{App, Bounds, Pixels, Point, point, px, size};

use crate::{
    bc,
    motion::{self, Curve, LINEAR, Motion, Track},
    shadows,
};

/// An overlay that grows in, as a system popover does, is done in 0.2 s, mostly in the first 0.1
/// (Arc 1.170 rec at 2×, the Space's icon picker: 0.57 of its size at 55 ms, 0.83 at 105 ms), and
/// the theme picker shrinks to its arrow and fades as it closes, gone in 0.11 s, as mac/host.mm's.
const GROW: f32 = 0.2;
const GROW_FADE: f32 = 0.1;
const GROW_CURVE: Curve = Curve(0.15, 0.75, 0.35, 1.);
const SHRINK: f32 = 0.11;
/// The size it grows from and shrinks to, about its growth point.
const SHRUNK: f32 = 0.05;

struct Overlay {
    view: *mut c_void,
    radius: f64,
    dismissed: extern "C" fn(),
    /// The height its UI asked for last, which it shows at; else its window's.
    height: Option<Pixels>,
    /// The point it grows in from, in it, and whether it shrinks back there as it goes.
    growth: Option<Point<Pixels>>,
    shrinks: bool,
    /// From host_set_overlay_motion, else a popover's.
    motion: Growth,
    /// When it started shrinking away, while it's still on screen.
    shrinking: Option<Instant>,
    shown: Option<Shown>,
}

/// The size an overlay grows from, over how long, how long its fade in takes, and how long it fades
/// out as it hides, or None for a popover's shrink.
#[derive(Clone, Copy)]
struct Growth {
    from: f32,
    seconds: f32,
    fade: f32,
    out: Option<f32>,
}

const POPOVER: Growth = Growth {
    from: SHRUNK,
    seconds: GROW,
    fade: GROW_FADE,
    out: None,
};

struct Shown {
    window: bc::bc_id,
    bounds: Bounds<Pixels>,
}

thread_local! {
    static OVERLAYS: RefCell<[Option<Overlay>; 5]> =
        const { RefCell::new([None, None, None, None, None]) };
    /// The app's overlays that have each browser window's keyboard, by their views.
    static LENT: RefCell<Vec<(bc::bc_id, usize)>> = const { RefCell::new(Vec::new()) };
    /// Reduce Motion, under which they grow and shrink no more, and only fade.
    static STILL: Cell<bool> = const { Cell::new(false) };
}

/// The size an overlay grows from and shrinks to, `from` unless Reduce Motion keeps it its own.
fn shrunk(from: f32) -> f32 {
    if STILL.get() { 1. } else { from }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_set_overlays_still(still: bool) {
    STILL.set(still);
}

/// The overlay of `view` takes the keyboard of `window`, or gives it back. The core lends it while
/// any has it, as the find bar can keep it under the command bar.
pub(crate) fn lend_keyboard(window: bc::bc_id, view: *mut c_void, takes: bool) {
    let entry = (window, view as usize);
    let (before, after) = LENT.with_borrow_mut(|lent| {
        let before = lent.iter().any(|&(each, _)| each == window);
        lent.retain(|&each| each != entry);
        if takes {
            lent.push(entry);
        }
        (before, lent.iter().any(|&(each, _)| each == window))
    });
    if before != after {
        unsafe { bc::bc_window_overlay_takes_keyboard(window, after) };
    }
    // The keys typed in the page since the shortcut that opened it are its.
    if takes {
        crate::give_held_keys(view);
        // Wayland leaves GPUI's window active under it, so a field there would stay open.
        crate::ui_keyboard_left_in(window);
    }
}

/// Whether an overlay but that of `view` has the keyboard of `window`.
pub(crate) fn keyboard_lent_to_another(window: bc::bc_id, view: *mut c_void) -> bool {
    LENT.with_borrow(|lent| {
        lent.iter()
            .any(|&(each, other)| each == window && other != view as usize)
    })
}

/// The overlays by index, as check.sh reads them in the log.
/// The icon and theme pickers and the Boost editor, host.h's HOST_ICON_PICKER, HOST_THEME_PICKER
/// and HOST_BOOST_EDITOR.
const PICKERS: [c_int; 3] = [1, 2, 4];

const NAMES: [&str; 5] = [
    "command bar",
    "icon picker",
    "theme picker",
    "folder preview",
    "boost editor",
];

fn with_overlay<R>(overlay: c_int, f: impl FnOnce(&mut Overlay) -> R) -> Option<R> {
    OVERLAYS.with_borrow_mut(|overlays| overlays.get_mut(overlay as usize)?.as_mut().map(f))
}

/// Shows `view` over the window whose UI is `ui_view`, as gpui_linux::show_overlay does, unless the
/// quit has gone ahead: its window is going, and under Wayland a subsurface made on a surface
/// Chromium has destroyed is a protocol error, which ends the connection.
pub(crate) fn show_over(
    view: *mut c_void,
    ui_view: *mut c_void,
    bounds: Bounds<Pixels>,
    radius: Pixels,
    keyboard: bool,
    left: fn(*mut c_void),
) {
    if !browser::shell::quit_gone_ahead() {
        gpui_linux::show_overlay(view, ui_view, bounds, radius, keyboard, left);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_overlay(
    overlay: c_int,
    view: *mut c_void,
    radius: f64,
    dismissed: extern "C" fn(),
) {
    OVERLAYS.with_borrow_mut(|overlays| {
        if let Some(slot) = overlays.get_mut(overlay as usize) {
            *slot = Some(Overlay {
                view,
                radius,
                dismissed,
                height: None,
                growth: None,
                shrinks: false,
                motion: POPOVER,
                shrinking: None,
                shown: None,
            });
        }
    });
    shadows::attach(overlay, view);
    motion::attach(view);
    if PICKERS.contains(&overlay) {
        motion::blur(view);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_set_overlay_radius(overlay: c_int, radius: f64) {
    with_overlay(overlay, |o| o.radius = radius);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_set_overlay_shape(overlay: c_int, corners: *const f64, count: usize) {
    if corners.is_null() {
        return;
    }
    // SAFETY: the caller passes `count` corners of three numbers each.
    let numbers = unsafe { std::slice::from_raw_parts(corners, count * 3) };
    let corners: Vec<gpui_linux::Corner> = (numbers.chunks_exact(3))
        .map(|corner| (corner[0] as f32, corner[1] as f32, corner[2] as f32))
        .collect();
    if let Some(view) = with_overlay(overlay, |o| o.view) {
        gpui_linux::set_overlay_outline(view, &corners);
        shadows::set_shape(overlay, &corners);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_set_overlay_growth(overlay: c_int, x: f64, y: f64) {
    with_overlay(overlay, |o| {
        o.growth = Some(point(px(x as f32), px(y as f32)))
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn host_set_overlay_shrink(overlay: c_int) {
    with_overlay(overlay, |o| o.shrinks = true);
}

#[unsafe(no_mangle)]
pub extern "C" fn host_set_overlay_motion(overlay: c_int, from: f64, seconds: f64, out: f64) {
    let (from, seconds, out) = (from as f32, seconds as f32, out as f32);
    let out = (out > 0.).then_some(out);
    with_overlay(overlay, |o| {
        o.motion = Growth {
            from,
            seconds,
            fade: seconds,
            out,
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn host_show_overlay(
    overlay: c_int,
    ui_view: *mut c_void,
    centered: bool,
    left: f64,
    top: f64,
    width: f64,
) {
    // The others close, as the user leaving them.
    for other in (0..NAMES.len() as c_int).filter(|&other| other != overlay) {
        if hide(other) {
            with_overlay(other, |other| later(other.dismissed));
        }
    }
    let Some(content) = gpui_linux::embedder_size(ui_view) else {
        return;
    };
    let (content_width, content_height) = (f64::from(content.width), f64::from(content.height));
    let (left, top, width) = match centered {
        true => (
            (content_width - width) / 2.,
            content_height / 2. - top,
            width,
        ),
        false => (left, top, width.min(content_width - left - 8.)),
    };
    let window = crate::window_with_view(ui_view);
    let shown = with_overlay(overlay, |o| {
        let height = (o.height)
            .or_else(|| gpui_linux::overlay_size(o.view).map(|size| size.height))
            .unwrap_or_default();
        let bounds = Bounds::new(
            point(px(left as f32), px(top as f32)),
            size(px(width as f32), height),
        );
        let was_shown = o.shown.replace(Shown { window, bounds }).is_some();
        // Where it can't blend, the window the card grows or shrinks in would show opaque around it.
        let grows = o
            .growth
            .filter(|_| !was_shown && gpui_linux::overlays_blend());
        let shrinking = o.shrinking.take().is_some();
        (
            o.view,
            px(o.radius as f32),
            bounds,
            was_shown,
            grows,
            shrinking,
            o.motion,
        )
    });
    let Some((view, radius, bounds, was_shown, grows, shrinking, growth)) = shown else {
        return;
    };
    if !was_shown {
        lend_keyboard(window, view, true);
    }
    if shrinking {
        // GPUI still shows the one fading, without the keys, and a press as it went, such as the
        // one that shows it again, left it: shown anew, that leave isn't this showing's (#2237).
        gpui_linux::hide_overlay(view, || {});
        gpui_linux::set_overlay_pointer(view, None);
    }
    let Growth {
        from,
        seconds,
        fade,
        ..
    } = growth;
    let scale = Track {
        from: shrunk(from),
        to: 1.,
        seconds,
        curve: GROW_CURVE,
    };
    if let Some(origin) = grows {
        let opacity = Track {
            from: 0.,
            to: 1.,
            seconds: fade,
            curve: LINEAR,
        };
        motion::start(
            view,
            Motion {
                scale,
                origin,
                opacity,
            },
        );
    }
    shadows::show(overlay, ui_view, bounds, f32::from(radius));
    if let Some(origin) = grows {
        let opacity = Track {
            from: 0.,
            to: 1.,
            seconds,
            curve: LINEAR,
        };
        shadows::follow(
            overlay,
            Motion {
                scale,
                origin,
                opacity,
            },
        );
    }
    show_over(view, ui_view, bounds, radius, true, left_overlay);
    let (origin, size) = (bounds.origin, bounds.size);
    let name = NAMES[overlay as usize];
    let at = [origin.x, origin.y, size.width, size.height].map(f32::from);
    browser::log!("browser: {name} at {} {} {} {}", at[0], at[1], at[2], at[3]);
    if !was_shown {
        browser::log!("browser: {name} shown");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_set_overlay_height(overlay: c_int, height: f64) {
    let moved = with_overlay(overlay, |o| {
        o.height = Some(px(height as f32));
        let shown = o.shown.as_mut()?;
        shown.bounds.size.height = px(height as f32);
        Some((o.view, shown.bounds, px(o.radius as f32)))
    });
    if let Some(Some((view, bounds, radius))) = moved {
        gpui_linux::move_overlay(view, bounds, radius);
        shadows::moved(overlay, bounds, f32::from(radius));
        browser::log!("browser: {} height {height}", NAMES[overlay as usize]);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_overlay(overlay: c_int) {
    hide(overlay);
}

/// Wayland lets no window move by itself, so the bar hides where it is and the card comes in.
#[unsafe(no_mangle)]
pub extern "C" fn host_fold_overlay(
    overlay: c_int,
    _x: f64,
    _y: f64,
    _width: f64,
    _height: f64,
    _radius: f64,
    _from: u32,
    _to: u32,
    _duration: f64,
    _fade: f64,
    _fade_only: bool,
) -> bool {
    hide(overlay);
    false
}

#[unsafe(no_mangle)]
pub extern "C" fn host_dismiss_overlay(overlay: c_int) {
    if hide(overlay) {
        with_overlay(overlay, |o| later(o.dismissed));
    }
}

/// The views of the overlays that show, the dialog's among them, for the test driver (drive.rs).
pub(crate) fn shown() -> Vec<*mut c_void> {
    let mut views: Vec<_> = OVERLAYS.with_borrow(|overlays| {
        (overlays.iter().flatten())
            .filter(|o| o.shown.is_some())
            .map(|o| o.view)
            .collect()
    });
    views.extend(DIALOG.with_borrow(|dialog| dialog.over.map(|_| dialog.view)));
    views
}

/// The browser window the overlay, dialog, panel or popover of `view` shows over, by its UI's view,
/// and where in its content, if it shows: ilo's menu shows over it there (menu_window.rs), as a
/// field's in it does.
pub(crate) fn placed(view: *mut c_void) -> Option<(*mut c_void, Bounds<Pixels>)> {
    let overlay = OVERLAYS.with_borrow(|overlays| {
        (overlays.iter().flatten())
            .find(|o| o.view == view)
            .and_then(|o| o.shown.as_ref().map(|shown| (shown.window, shown.bounds)))
    });
    if let Some((window, bounds)) = overlay {
        let ui_view = crate::WINDOWS.with_borrow(|windows| {
            windows
                .iter()
                .find(|each| each.id == window)
                .map(|each| each.ui_view)
        })?;
        return Some((ui_view, bounds));
    }
    let dialog = DIALOG.with_borrow(|dialog| {
        (dialog.over.filter(|_| dialog.view == view)).map(|(ui_view, _)| (ui_view, dialog.insets))
    });
    if let Some((ui_view, insets)) = dialog {
        return dialog_bounds(ui_view, insets).map(|(bounds, _)| (ui_view, bounds));
    }
    crate::panels::placed(view).or_else(|| crate::popovers::placed(view))
}

/// Overlay `overlay`'s view and where it shows in its browser window's content, if it shows, for a
/// tooltip over it (floats.rs).
pub(crate) fn shown_at(overlay: c_int) -> Option<(*mut c_void, Bounds<Pixels>)> {
    with_overlay(overlay, |o| {
        o.shown.as_ref().map(|shown| (o.view, shown.bounds))
    })
    .flatten()
}

/// Puts overlay `overlay` where it shows again, if it does, which stacks it and what shows under it
/// again: its shadow under it, and it under a tooltip that has come over it (floats.rs).
pub(crate) fn restack(overlay: c_int) {
    let shown = with_overlay(overlay, |o| {
        o.shown
            .as_ref()
            .map(|shown| (o.view, shown.bounds, px(o.radius as f32)))
    });
    if let Some(Some((view, bounds, radius))) = shown {
        gpui_linux::move_overlay(view, bounds, radius);
    }
}

/// Hides `overlay` if it shows, and says whether it did. The window has its keyboard back once
/// the overlay's gone, as the input focus on X11 says who had it.
fn hide(overlay: c_int) -> bool {
    // A tooltip over it goes with it, at once, not left over the page as it fades.
    crate::floats::overlay_hiding(overlay);
    let Some(Some((view, window, shrinks, growth))) = with_overlay(overlay, |o| {
        let shown = o.shown.take()?;
        let goes = o.shrinks || o.motion.out.is_some();
        let shrinks = o.growth.filter(|_| goes && gpui_linux::overlays_blend());
        o.shrinking = shrinks.map(|_| Instant::now());
        Some((
            o.view,
            shown.window,
            shrinks.zip(o.shrinking),
            (o.shrinks, o.motion),
        ))
    }) else {
        return false;
    };
    browser::log!("browser: {} hidden", NAMES[overlay as usize]);
    let Some((origin, started)) = shrinks else {
        gpui_linux::hide_overlay(view, move || lend_keyboard(window, view, false));
        shadows::hide(overlay);
        return true;
    };
    // What it shows stays as it shrinks, and the clicks and keys go to what's under it.
    gpui_linux::set_overlay_pointer(view, Some(&[]));
    gpui_linux::set_overlay_keyboard(view, false, left_overlay);
    lend_keyboard(window, view, false);
    // It fades where it is, unless it shrinks back.
    let (shrinks, Growth { from, out, .. }) = growth;
    let (to, seconds) = (
        if shrinks { shrunk(from) } else { 1. },
        out.unwrap_or(SHRINK),
    );
    let scale = Track {
        from: 1.,
        to,
        seconds,
        curve: motion::EASE_IN,
    };
    let opacity = Track {
        from: 1.,
        to: 0.,
        seconds,
        curve: motion::EASE_IN,
    };
    motion::start(
        view,
        Motion {
            scale,
            origin,
            opacity,
        },
    );
    let opacity = Track {
        from: 1.,
        to: 0.,
        seconds,
        curve: LINEAR,
    };
    shadows::follow(
        overlay,
        Motion {
            scale,
            origin,
            opacity,
        },
    );
    motion::after(seconds, move || {
        // Unless it showed again as it shrank.
        let gone = with_overlay(overlay, |o| {
            o.shrinking
                .take_if(|shrinking| *shrinking == started)
                .is_some()
        });
        if gone == Some(true) {
            gpui_linux::hide_overlay(view, || {});
            gpui_linux::set_overlay_pointer(view, None);
            shadows::hide(overlay);
        }
    });
    true
}

/// The user left the overlay of `view`: a press outside it, or the keyboard going elsewhere. GPUI
/// says so once its events are dispatched, outside any update.
fn left_overlay(view: *mut c_void) {
    // To ilo's menu over it, such as its field's, which gives the keyboard back as it closes.
    if crate::menu_window::over(view) {
        gpui_linux::set_overlay_keyboard(view, true, left_overlay);
        return;
    }
    let overlay = OVERLAYS.with_borrow(|overlays| {
        overlays
            .iter()
            .position(|o| o.as_ref().is_some_and(|o| o.view == view))
    });
    if let Some(overlay) = overlay
        && hide(overlay as c_int)
        && let Some(dismissed) = with_overlay(overlay as c_int, |o| o.dismissed)
    {
        dismissed();
    }
}

/// Runs `dismissed` once the update under way, if any, is over.
/// Runs `f` on GPUI's window whose view is `view`, if it's open.
pub(crate) fn update_window(
    view: *mut c_void,
    cx: &mut App,
    f: impl FnOnce(&mut gpui::Window, &mut App),
) {
    let mut f = Some(f);
    for window in cx.windows() {
        let done = window.update(cx, |_, window, cx| {
            if crate::ns_view(window) != view {
                return false;
            }
            if let Some(f) = f.take() {
                f(window, cx);
            }
            true
        });
        if let Ok(true) = done {
            return;
        }
    }
}

pub(crate) fn later(dismissed: extern "C" fn()) {
    if let Some(cx) = crate::CX.with_borrow(Clone::clone) {
        cx.foreground_executor()
            .spawn(async move { dismissed() })
            .detach();
    }
}

/// The dialog's view, and while it shows, the browser window it's over (its UI's view and the
/// core's id) and how far it's inset from that one's content's edges, left, top, right and bottom:
/// a page's over its tab's card, the rest over all of it.
struct Dialog {
    view: *mut c_void,
    over: Option<(*mut c_void, bc::bc_id)>,
    insets: [f64; 4],
    /// Answered, it draws as it leaves with the keyboard back with the browser window.
    leaving: bool,
}

thread_local! {
    static DIALOG: RefCell<Dialog> = const {
        RefCell::new(Dialog {
            view: std::ptr::null_mut(),
            over: None,
            insets: [0.; 4],
            leaving: false,
        })
    };
}

/// Where the dialog goes in the content of the browser window of `ui_view`, and its corners: a
/// tab's card's, or the window's.
fn dialog_bounds(ui_view: *mut c_void, insets: [f64; 4]) -> Option<(Bounds<Pixels>, Pixels)> {
    let content = gpui_linux::embedder_size(ui_view)?;
    let [left, top, right, bottom] = insets.map(|inset| px(inset as f32));
    let bounds = Bounds::new(
        point(left, top),
        size(content.width - left - right, content.height - top - bottom),
    );
    let card = insets != [0.; 4];
    Some((
        bounds,
        px(if card {
            browser::shell::CARD_RADIUS
        } else {
            0.
        }),
    ))
}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_dialog(dialog_view: *mut c_void) {
    DIALOG.with_borrow_mut(|dialog| dialog.view = dialog_view);
}

/// Shows the dialog over all of the browser window of `ui_view`, or over the tab's card at `x`,
/// `y`, `width` and `height` in its content, with the keyboard, as it's answered before anything
/// else. Asked again, it goes where the card went, such as when the sidebar hid.
#[unsafe(no_mangle)]
pub extern "C" fn host_show_dialog(ui_view: *mut c_void, x: f64, y: f64, width: f64, height: f64) {
    let Some(content) = gpui_linux::embedder_size(ui_view) else {
        return;
    };
    let insets = match width > 0. {
        true => [
            x,
            y,
            f64::from(content.width) - x - width,
            f64::from(content.height) - y - height,
        ],
        false => [0.; 4],
    };
    let (view, over) = DIALOG.with_borrow_mut(|dialog| {
        if dialog.over.is_none_or(|(over, _)| over == ui_view) {
            dialog.insets = insets;
        }
        (dialog.view, dialog.over)
    });
    if view.is_null() {
        return;
    }
    if over.is_some() {
        place_dialog(ui_view);
        return;
    }
    let Some((bounds, radius)) = dialog_bounds(ui_view, insets) else {
        return;
    };
    let window = crate::window_with_view(ui_view);
    DIALOG.with_borrow_mut(|dialog| {
        dialog.over = Some((ui_view, window));
        dialog.leaving = false;
    });
    lend_keyboard(window, view, true);
    gpui_linux::set_overlay_pointer(view, None);
    show_over(view, ui_view, bounds, radius, true, left_dialog);
    browser::log!("browser: dialog window shown");
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_dialog() {
    let (view, over) = DIALOG.with_borrow_mut(|dialog| (dialog.view, dialog.over.take()));
    let Some((_, window)) = over else {
        return;
    };
    gpui_linux::hide_overlay(view, move || {
        // The next question, which shows as this one hides, keeps the keyboard it took.
        if DIALOG.with_borrow(|dialog| dialog.over.is_none()) {
            lend_keyboard(window, view, false);
        }
    });
    browser::log!("browser: dialog window hidden");
}

/// The dialog was answered and leaves: it goes on drawing, and the clicks and keys go to the
/// browser window under it at once, to the page or the UI that had them, as a key typed while it
/// leaves, such as ⌥⌘N right after Esc, would otherwise reach the dialog going (#1963).
pub(crate) fn dialog_leaving() {
    let left = DIALOG.with_borrow_mut(|dialog| {
        let (_, window) = dialog.over.filter(|_| !dialog.leaving)?;
        dialog.leaving = true;
        Some((dialog.view, window))
    });
    let Some((view, window)) = left else {
        return;
    };
    gpui_linux::set_overlay_pointer(view, Some(&[]));
    gpui_linux::set_overlay_keyboard(view, false, left_dialog);
    lend_keyboard(window, view, false);
    browser::log!("browser: dialog gives the keyboard back");
}

/// Places the dialog again if it shows over the browser window of `ui_view`, as its content
/// changed size.
pub(crate) fn place_dialog(ui_view: *mut c_void) {
    let (view, over, insets) =
        DIALOG.with_borrow(|dialog| (dialog.view, dialog.over, dialog.insets));
    if over.is_some_and(|(over, _)| over == ui_view)
        && let Some((bounds, radius)) = dialog_bounds(ui_view, insets)
    {
        gpui_linux::move_overlay(view, bounds, radius);
    }
}

/// Hides the dialog if it shows over the browser window of `ui_view`, which is closing.
pub(crate) fn forget_window(ui_view: *mut c_void) {
    if DIALOG.with_borrow(|dialog| dialog.over.is_some_and(|(over, _)| over == ui_view)) {
        host_hide_dialog();
    }
}

/// The user left the dialog, such as for the sidebar beside a page's, or Chromium took its
/// window's keyboard as a page committed: it takes the keyboard back, unless another app has it.
fn left_dialog(view: *mut c_void) {
    // One leaving has given the keyboard back for good.
    if DIALOG.with_borrow(|dialog| dialog.leaving) {
        return;
    }
    // To ilo's menu over it, its field's, which gives the keyboard back as it closes.
    if crate::menu_window::over(view) {
        gpui_linux::set_overlay_keyboard(view, true, left_dialog);
        return;
    }
    if DIALOG.with_borrow(|dialog| dialog.over.is_some()) {
        gpui_linux::take_overlay_keyboard(view, left_dialog);
    }
}
