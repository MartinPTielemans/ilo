//! Where the browser windows are, so a relaunch brings them back where they were and in their order
//! (host.h's host_window_frame, host_screens, host_cascaded and host_show_window_behind). On X11
//! the X server says, as the driver reads it. Wayland shows no client a window's place or the
//! screens', nor lets one stack its windows, so there the windows come back at their sizes,
//! wherever the compositor puts them.

use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    rc::Rc,
};

use x11rb::{
    connection::Connection as _,
    protocol::{
        randr::ConnectionExt as _,
        xproto::{self, ConnectionExt as _},
    },
    rust_connection::RustConnection,
};

use crate::{WINDOWS, bc};

/// How far down and right ⌘N's window opens from the one in front: about a title bar, as AppKit
/// and Windows cascade.
const CASCADE: f64 = 22.;

thread_local! {
    static X: RefCell<Option<Rc<(RustConnection, xproto::Window)>>> = const { RefCell::new(None) };
    /// Whether the browser windows are X11's, once one has said.
    static ON_X11: Cell<Option<bool>> = const { Cell::new(None) };
}

fn server() -> Option<Rc<(RustConnection, xproto::Window)>> {
    X.with_borrow_mut(|x| {
        if x.is_none() {
            let (connection, screen) = x11rb::connect(None).ok()?;
            let root = connection.setup().roots[screen].root;
            *x = Some(Rc::new((connection, root)));
        }
        x.clone()
    })
}

/// The core's id for the browser window whose UI draws in `ui_view`, while it's open.
fn window_of(ui_view: *mut c_void) -> Option<bc::bc_id> {
    // Asked while a window closes, which may hold the list.
    WINDOWS.with(|windows| {
        let windows = windows.try_borrow().ok()?;
        windows.iter().find(|w| w.ui_view == ui_view).map(|w| w.id)
    })
}

/// Window `id`'s size in points under Wayland, and None on X11.
fn wayland_size(id: bc::bc_id) -> Option<[f64; 2]> {
    let (mut width, mut height) = (0., 0.);
    let surface = unsafe { bc::bc_window_wayland_surface(id, &mut width, &mut height) };
    ON_X11.set(Some(surface.is_null()));
    (!surface.is_null()).then_some([width, height])
}

/// Whether the browser windows are X11's: None before the first window, when nothing has said
/// which Chromium went to, and a connection could start a compositor's Xwayland.
fn on_x11() -> Option<bool> {
    if ON_X11.get().is_none() {
        if let Some(first) = WINDOWS.with(|w| w.try_borrow().ok()?.first().map(|w| w.id)) {
            wayland_size(first);
        }
    }
    ON_X11.get()
}

/// The browser window's frame in points from the screen's top left, as the core keeps it. Under
/// Wayland its size alone, at 0, 0. On X11 a window closing, the last of them, still has its own.
fn frame(ui_view: *mut c_void) -> Option<[f64; 4]> {
    if let Some([width, height]) = window_of(ui_view).and_then(wayland_size) {
        return Some([0., 0., width, height]);
    }
    // Under Wayland the view is a surface, which as an X id could name another client's window.
    (on_x11() == Some(true)).then_some(())?;
    let x = server()?;
    let (connection, root) = &*x;
    // GPUI's window is in Chromium's, the browser window.
    let window = connection
        .query_tree(ui_view as usize as u32)
        .ok()?
        .reply()
        .ok()?
        .parent;
    let geometry = connection.get_geometry(window).ok()?.reply().ok()?;
    let at = (connection.translate_coordinates(window, *root, 0, 0))
        .ok()?
        .reply()
        .ok()?;
    let scale = unsafe { bc::bc_display_scale() };
    let frame = [
        at.dst_x.into(),
        at.dst_y.into(),
        geometry.width.into(),
        geometry.height.into(),
    ];
    Some(frame.map(|n: f64| n / scale))
}

/// Left be while maximized or full screen, as on the Mac and Windows: the window comes back from
/// those to the frame the core kept.
#[unsafe(no_mangle)]
pub extern "C" fn host_window_frame(
    ui_view: *mut c_void,
    x: *mut f64,
    y: *mut f64,
    width: *mut f64,
    height: *mut f64,
) -> bool {
    if let Some(id) = window_of(ui_view)
        && unsafe { bc::bc_window_is_maximized(id) || bc::bc_window_is_full_screen(id) }
    {
        return false;
    }
    let Some([left, top, wide, high]) = frame(ui_view) else {
        return false;
    };
    unsafe { (*x, *y, *width, *height) = (left, top, wide, high) };
    true
}

/// The X server's monitors, the primary first, else the whole screen. None under Wayland, nor
/// before the first window (on_x11).
#[unsafe(no_mangle)]
pub extern "C" fn host_screens(frames: *mut f64, count: usize) -> usize {
    if on_x11() != Some(true) {
        return 0;
    }
    let Some(x) = server() else {
        return 0;
    };
    let (connection, root) = &*x;
    let mut screens: Vec<[f64; 4]> = (connection.randr_get_monitors(*root, true).ok())
        .and_then(|cookie| cookie.reply().ok())
        .map(|reply| {
            let mut monitors = reply.monitors;
            monitors.sort_by_key(|monitor| !monitor.primary);
            let frame = |m: &x11rb::protocol::randr::MonitorInfo| {
                [m.x.into(), m.y.into(), m.width.into(), m.height.into()]
            };
            monitors.iter().map(frame).collect()
        })
        .unwrap_or_default();
    if screens.is_empty() {
        let Some(geometry) = (connection.get_geometry(*root).ok()).and_then(|c| c.reply().ok())
        else {
            return 0;
        };
        screens.push([0., 0., geometry.width.into(), geometry.height.into()]);
    }
    let scale = unsafe { bc::bc_display_scale() };
    for (i, screen) in screens.iter().take(count).enumerate() {
        for (j, n) in screen.iter().enumerate() {
            unsafe { *frames.add(i * 4 + j) = n / scale };
        }
    }
    screens.len()
}

/// As big as the window of `ui_view`, and on X11 down and to its right. Under Wayland the
/// compositor places it.
#[unsafe(no_mangle)]
pub extern "C" fn host_cascaded(
    ui_view: *mut c_void,
    x: *mut f64,
    y: *mut f64,
    width: *mut f64,
    height: *mut f64,
) {
    let Some([left, top, wide, high]) = frame(ui_view) else {
        return;
    };
    unsafe { (*x, *y, *width, *height) = (left + CASCADE, top + CASCADE, wide, high) };
}

/// A saved window comes back behind the one in front of it. The core shows it without the keyboard,
/// which on X11 still maps it on top, so it's put under that window by the window manager, which
/// keeps the stacking (EWMH's _NET_RESTACK_WINDOW). Chromium flushes the map before the core
/// returns, so the window manager has the window by the time this reaches it.
#[unsafe(no_mangle)]
pub extern "C" fn host_show_window_behind(ui_view: *mut c_void, above_ui_view: *mut c_void) {
    put_under(ui_view, above_ui_view);
}

fn put_under(ui_view: *mut c_void, above_ui_view: *mut c_void) -> Option<()> {
    (on_x11() == Some(true)).then_some(())?;
    let x = server()?;
    let (connection, root) = &*x;
    // GPUI's window is in Chromium's, the browser window.
    let window = |view: *mut c_void| {
        Some(
            connection
                .query_tree(view as usize as u32)
                .ok()?
                .reply()
                .ok()?
                .parent,
        )
    };
    let (window, above) = (window(ui_view)?, window(above_ui_view)?);
    let restack = connection
        .intern_atom(false, b"_NET_RESTACK_WINDOW")
        .ok()?
        .reply()
        .ok()?
        .atom;
    // From a pager, source 2, which is the one Openbox takes it from.
    let below = u32::from(xproto::StackMode::BELOW);
    let message = xproto::ClientMessageEvent::new(32, window, restack, [2, above, below, 0, 0]);
    let mask = xproto::EventMask::SUBSTRUCTURE_REDIRECT | xproto::EventMask::SUBSTRUCTURE_NOTIFY;
    connection.send_event(false, *root, mask, message).ok()?;
    connection.flush().ok()
}
