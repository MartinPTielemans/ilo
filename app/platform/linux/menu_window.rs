//! ilo's own context menu's window (src/menu.rs) on Linux, as mac/menu_window.mm is on the Mac:
//! GPUI's Linux platform shows it over the window it opened for as one of its overlays
//! (overlays.rs), with the keyboard, and the menu's own window takes its pointer events. A press
//! outside, the keyboard going elsewhere or the window resizing is the user leaving it.
//!
//! A right-click elsewhere in the UI opens that menu in its place, as the press reaches the UI
//! before GPUI tells the menu it was left, and a leaving is told only to the showing it left.
//!
//! shortcut: the pointer isn't the menu's first, so a press outside closes it and lands where it
//! was pressed too. It stays inside its window, which it can't go past under Wayland.
//!
//! From the Linux driver's branch (feat/linux-drive, a566b45), which shows it the same way.

use std::{
    cell::Cell,
    ffi::{c_char, c_void},
};

use gpui::{Bounds, Pixels, Size, point, px, size};

use crate::{bc, overlays};

struct Menu {
    view: *mut c_void,
    dismissed: extern "C" fn(),
    choose: extern "C" fn(*const c_char) -> bool,
}

thread_local! {
    static MENU: Cell<Option<&'static Menu>> = const { Cell::new(None) };
    /// The window it shows over, by its UI's view and the core's id, 0 for Settings, and that
    /// window's size as it showed; None while hidden.
    static SHOWN: Cell<Option<(*mut c_void, bc::bc_id, Size<Pixels>)>> = const { Cell::new(None) };
    /// The overlay it opened from, such as the command bar for its field's menu, by its view.
    static OVER: Cell<Option<*mut c_void>> = const { Cell::new(None) };
}

#[unsafe(no_mangle)]
pub extern "C" fn host_add_menu_window(
    view: *mut c_void,
    _pointer: extern "C" fn(kind: i32, x: f64, y: f64) -> bool,
    dismissed: extern "C" fn(),
    choose: extern "C" fn(*const c_char) -> bool,
) {
    gpui_linux::set_overlay_menu(view);
    MENU.set(Some(Box::leak(Box::new(Menu {
        view,
        dismissed,
        choose,
    }))));
}

/// The menu's window takes the pointer on its `panels`, from its top left, and lets it through to
/// the browser window in their shadows' room round them, where a press is outside the menu. Where
/// overlays can't blend, it's also cut to them, as the room round them would show black (#1479).
pub(crate) fn set_panels(panels: &[Bounds<Pixels>]) {
    let Some(menu) = MENU.get() else { return };
    gpui_linux::set_overlay_pointer(menu.view, Some(panels));
    if !gpui_linux::overlays_blend() {
        let square = |panel: &Bounds<Pixels>| {
            let (left, top) = (f32::from(panel.left()), f32::from(panel.top()));
            let (right, bottom) = (f32::from(panel.right()), f32::from(panel.bottom()));
            vec![
                (left, top, 0.),
                (right, top, 0.),
                (right, bottom, 0.),
                (left, bottom, 0.),
            ]
        };
        gpui_linux::set_overlay_outlines(menu.view, &panels.iter().map(square).collect::<Vec<_>>());
    }
}

/// Shows the menu's window at `left`, `top`, `width`, `height` in the content of `ui_view`'s
/// browser window, of the overlay `ui_view` is, or of Settings, with its keyboard, or moves it
/// there as a submenu opens.
#[unsafe(no_mangle)]
pub extern "C" fn host_show_menu_window(
    ui_view: *mut c_void,
    _in_reveal: bool,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
) {
    let Some(menu) = MENU.get() else { return };
    let (over, (ui_view, origin)) = match overlays::placed(ui_view) {
        Some((browser, bounds)) => (Some(ui_view), (browser, bounds.origin)),
        None => (None, (ui_view, point(px(0.), px(0.)))),
    };
    // Settings is GPUI's own window, not the core's, and the menu takes its keys from it itself.
    let window = crate::window_with_view(ui_view);
    if window == 0 && crate::settings_window::shown() != Some(ui_view) {
        return;
    }
    if SHOWN.get().is_none() {
        let content = gpui_linux::embedder_size(ui_view).unwrap_or_default();
        SHOWN.set(Some((ui_view, window, content)));
        OVER.set(over);
        if window != 0 {
            overlays::lend_keyboard(window, menu.view, true);
        }
    }
    let bounds = Bounds::new(
        origin + point(px(left as f32), px(top as f32)),
        size(px(width as f32), px(height as f32)),
    );
    // The menu draws its panels' corners and shadows inside its window.
    crate::overlays::show_over(menu.view, ui_view, bounds, px(0.), true, left_menu);
    // check.sh reads it.
    browser::log!("browser: menu window at {left:.0},{top:.0} {width:.0}x{height:.0}");
}

#[unsafe(no_mangle)]
pub extern "C" fn host_hide_menu_window() {
    let (Some(menu), Some((_, window, _))) = (MENU.get(), SHOWN.take()) else {
        return;
    };
    OVER.take();
    let view = menu.view;
    gpui_linux::hide_overlay(view, move || {
        // A menu that opened again in the window before this hide ran keeps the keyboard it took.
        let again = SHOWN.get().is_some_and(|(_, shown, _)| shown == window);
        if window != 0 && !again {
            overlays::lend_keyboard(window, view, false);
        }
    });
    browser::log!("browser: menu window hidden");
}

/// The window's content, in the points of `ui_view`, its own or an overlay's over it: the menu
/// stays inside it.
#[unsafe(no_mangle)]
pub extern "C" fn host_menu_area(ui_view: *mut c_void, _in_reveal: bool, area: *mut f64) {
    let (ui_view, origin) = overlays::placed(ui_view)
        .map_or((ui_view, point(px(0.), px(0.))), |(browser, bounds)| {
            (browser, bounds.origin)
        });
    let content = gpui_linux::embedder_size(ui_view).unwrap_or_default();
    let (x, y) = (f64::from(origin.x), f64::from(origin.y));
    let edges = [
        -x,
        -y,
        f64::from(content.width) - x,
        f64::from(content.height) - y,
    ];
    // SAFETY: the caller passes four.
    unsafe { std::slice::from_raw_parts_mut(area, 4) }.copy_from_slice(&edges);
}

/// The browser window of `ui_view` is closing: the menu over it closes, once GPUI is outside its
/// updates.
pub(crate) fn forget_window(ui_view: *mut c_void) {
    if SHOWN.get().is_some_and(|(over, _, _)| over == ui_view)
        && let Some(menu) = MENU.get()
    {
        gpui_linux::leave_overlay(menu.view);
    }
}

/// GPUI says `ui_view`'s window bounds changed, as it also does when the window takes the
/// keyboard, which the press that opens a menu gives it: the menu closes if the size did, as the
/// Mac's does.
pub(crate) fn resized(ui_view: *mut c_void) {
    if let Some((over, _, content)) = SHOWN.get()
        && over == ui_view
        && gpui_linux::embedder_size(ui_view).unwrap_or_default() != content
    {
        forget_window(ui_view);
    }
}

/// Whether the menu shows over the overlay of `view`, having opened from it.
pub(crate) fn over(view: *mut c_void) -> bool {
    SHOWN.get().is_some() && OVER.get() == Some(view)
}

/// The menu's view while it shows, for the test driver (drive.rs).
pub(crate) fn shown() -> Option<*mut c_void> {
    SHOWN.get().and(MENU.get()).map(|menu| menu.view)
}

/// For the driver: chooses `title`, or "Submenu › Item", in the menu that shows. Returns whether
/// a menu was showing with that item.
pub(crate) fn choose(title: *const c_char) -> bool {
    shown().is_some() && MENU.get().is_some_and(|menu| (menu.choose)(title))
}

/// For the driver: closes the menu that shows, as the user leaving it. Returns whether one did.
pub(crate) fn dismiss() -> bool {
    let Some(view) = shown() else { return false };
    gpui_linux::leave_overlay(view);
    true
}

/// The user left the menu. GPUI says so once its events are dispatched, outside any update.
fn left_menu(_: *mut c_void) {
    if SHOWN.get().is_some()
        && let Some(menu) = MENU.get()
    {
        (menu.dismissed)();
    }
}
