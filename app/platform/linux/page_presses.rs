//! A press in a browser window's page on X11, for a click in an agent's page to take it over
//! (Shell::page_pressed, #1080). The pages are Chromium's to draw in its window, whose presses the X
//! server gives Chromium's connection alone, so while an agent works (`hear`) a connection of the
//! host's own hears the screen's raw presses, which say no place, and reads the pointer's as each
//! comes: our browser window, on top there, and not GPUI's window in it, is a page. It stops once
//! no agent works, at the next press. Under Wayland GPUI hears the page's presses itself (main.rs's
//! `page_pressed`).
use std::{cell::Cell, sync::Arc};

use gpui::App;
use x11rb::{
    connection::Connection as _,
    errors::ReplyError,
    protocol::{
        Event,
        xinput::{self, ConnectionExt as _},
        xproto::{self, ConnectionExt as _},
    },
    rust_connection::RustConnection,
};

use crate::{WINDOWS, bc};

thread_local! {
    /// Which listener is the one wanted, 0 for none: a stopped one ends at its next press.
    static LISTENER: Cell<u64> = const { Cell::new(0) };
    static LISTENERS: Cell<u64> = const { Cell::new(0) };
}

/// Hears the presses in pages from now on, while `on`.
pub(crate) fn hear(on: bool, cx: &mut App) {
    if crate::frames::on_x11() != Some(true) {
        return;
    }
    match (on, LISTENER.get()) {
        (false, _) => return LISTENER.set(0),
        (true, 0) => {}
        (true, _) => return,
    }
    let Some((connection, root)) = connect() else {
        return;
    };
    let id = LISTENERS.get() + 1;
    LISTENERS.set(id);
    LISTENER.set(id);
    let connection = Arc::new(connection);
    // check.sh reads this line.
    browser::log!("browser: page presses heard");
    cx.spawn(async move |cx| {
        loop {
            let waiting = connection.clone();
            // Where the pointer is as the press comes, before it moves on.
            let pressed = (cx.background_executor())
                .spawn(async move { next_press(&waiting, root) })
                .await;
            if LISTENER.get() != id {
                break;
            }
            match pressed {
                Ok(Some((x, y))) => {
                    if let Some(window) = pressed_page(root, x, y) {
                        crate::page_pressed_in(window);
                    }
                }
                Ok(None) => {}
                Err(_) => break,
            }
        }
    })
    .detach();
}

/// Waits for the screen's next event: a press of a mouse button, not the wheel's, says where the
/// pointer is at it.
fn next_press(
    connection: &RustConnection,
    root: xproto::Window,
) -> Result<Option<(i16, i16)>, ReplyError> {
    let event = connection.wait_for_event()?;
    match event {
        Event::XinputRawButtonPress(press) if browser::agents::press_takes_over(press.detail) => {
            let pointer = connection.query_pointer(root)?.reply()?;
            Ok(Some((pointer.root_x, pointer.root_y)))
        }
        _ => Ok(None),
    }
}

/// A connection that hears the master devices' raw presses on the screen, and its root.
fn connect() -> Option<(RustConnection, xproto::Window)> {
    let (connection, screen) = RustConnection::connect(None).ok()?;
    let root = connection.setup().roots.get(screen)?.root;
    connection
        .xinput_xi_query_version(2, 2)
        .ok()?
        .reply()
        .ok()?;
    let mask = xinput::EventMask {
        deviceid: xinput::Device::ALL_MASTER.into(),
        mask: vec![xinput::XIEventMask::RAW_BUTTON_PRESS],
    };
    connection.xinput_xi_select_events(root, &[mask]).ok()?;
    connection.flush().ok()?;
    Some((connection, root))
}

/// The browser window whose page is at `x`, `y` on the screen, where a press was: inside the
/// window, under no other client's, and not GPUI's window in it.
fn pressed_page(root: xproto::Window, x: i16, y: i16) -> Option<bc::bc_id> {
    let (connection, _) = RustConnection::connect(None).ok()?;
    let top = (connection.translate_coordinates(root, root, x, y))
        .ok()?
        .reply()
        .ok()?
        .child;
    let views: Vec<(bc::bc_id, u32)> = WINDOWS.with(|windows| {
        let windows = windows.try_borrow().ok()?;
        Some(
            (windows.iter())
                .map(|window| (window.id, window.ui_view as usize as u32))
                .collect(),
        )
    })?;
    views.into_iter().find_map(|(id, ui)| {
        let window = connection.query_tree(ui).ok()?.reply().ok()?.parent;
        // The window manager's frame round it, or itself, is the top window there.
        let mut outer = window;
        loop {
            let parent = connection.query_tree(outer).ok()?.reply().ok()?.parent;
            if parent == root || parent == x11rb::NONE {
                break;
            }
            outer = parent;
        }
        (outer == top).then_some(())?;
        let at = (connection.translate_coordinates(root, window, x, y))
            .ok()?
            .reply()
            .ok()?;
        let size = connection.get_geometry(window).ok()?.reply().ok()?;
        let inside = (0..size.width as i16).contains(&at.dst_x)
            && (0..size.height as i16).contains(&at.dst_y);
        (inside && at.child != ui).then_some(id)
    })
}
