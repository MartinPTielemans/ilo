//! host.h's host_watch_window_main on Linux: whether each browser window is the one the user is in.
//!
//! On X11 that's the window manager's active window, which it keeps in the root's
//! `_NET_ACTIVE_WINDOW`, read as that changes (PropertyNotify), so nothing polls. It's Chromium's
//! window, which GPUI's is in, or a window transient for it. The overlays and menus take the
//! keyboard as override-redirect windows, which the window manager doesn't manage, and while one has
//! it, it says no window is active, which leaves each as it was. Settings, a window of its own,
//! takes it from the browser window.
//!
//! Under Wayland it's the browser window's having the keyboard, which the compositor gives the
//! activated toplevel and GPUI hears for the whole window, the page's keyboard or the UI's
//! (system.rs's watch_activation).

use std::{cell::RefCell, ffi::c_void, os::fd::AsRawFd as _, rc::Rc};

use x11rb::{
    connection::Connection as _,
    protocol::{
        Event,
        xproto::{self, AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, EventMask},
    },
    rust_connection::RustConnection,
};

use crate::{WINDOWS, frames, platform};

struct Watch {
    connection: RustConnection,
    root: xproto::Window,
    active: xproto::Atom,
}

thread_local! {
    static CHANGED: RefCell<Option<extern "C" fn(*mut c_void, bool)>> = const { RefCell::new(None) };
    /// The X server's, on X11, once the watch has started.
    static WATCH: RefCell<Option<Rc<Watch>>> = const { RefCell::new(None) };
    /// What each browser window was last told, by its UI's view.
    static TOLD: RefCell<Vec<(*mut c_void, bool)>> = const { RefCell::new(Vec::new()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn host_watch_window_main(changed: extern "C" fn(*mut c_void, bool)) {
    CHANGED.set(Some(changed));
    if frames::on_x11() == Some(true) && WATCH.with_borrow(Option::is_none) {
        WATCH.set(watch().map(Rc::new));
        if let Some(watch) = WATCH.with_borrow(Clone::clone) {
            let fd = watch.connection.stream().as_raw_fd();
            extern "C" fn ready() {
                pump();
            }
            unsafe { platform::host_watch_fd(fd, ready) };
        }
    }
    tell_all();
}

/// The browser window of `ui_view` has opened, which may be in front already.
pub(crate) fn window_opened() {
    tell_all();
}

/// Under Wayland, the browser window of `ui_view` has the keyboard, or has lost it.
pub(crate) fn wayland_active(ui_view: *mut c_void, active: bool) {
    tell(ui_view, active);
}

fn watch() -> Option<Watch> {
    let (connection, screen) = x11rb::connect(None).ok()?;
    let root = connection.setup().roots[screen].root;
    let active = connection
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")
        .ok()?
        .reply()
        .ok()?
        .atom;
    // PROBE #2762: also the _NET_ACTIVE_WINDOW requests clients send the window manager.
    let events = ChangeWindowAttributesAux::new()
        .event_mask(EventMask::PROPERTY_CHANGE | EventMask::SUBSTRUCTURE_NOTIFY);
    connection
        .change_window_attributes(root, &events)
        .ok()?
        .check()
        .ok()?;
    Some(Watch {
        connection,
        root,
        active,
    })
}

/// Reads the events the X server has sent, and once the active window has changed, tells the
/// windows. A reply read meanwhile can leave events read with it, which the socket no longer
/// signals, so it reads again till none is left.
fn pump() {
    let Some(watch) = WATCH.with_borrow(Clone::clone) else {
        return;
    };
    loop {
        let mut changed = false;
        while let Ok(Some(event)) = watch.connection.poll_for_event() {
            match event {
                Event::PropertyNotify(notify) => {
                    if notify.window == watch.root && notify.atom == watch.active {
                        changed = true;
                        let active = window_in(&watch, watch.root, watch.active);
                        crate::log!("browser: probe active now {:#x} at {}, focus {}", active.unwrap_or(0), notify.time, probe_focus(&watch));
                    }
                }
                Event::ClientMessage(message) if message.type_ == watch.active => {
                    let data = message.data.as_data32();
                    crate::log!("browser: probe active asked for {:#x} source {} time {} current {:#x}", message.window, data[0], data[1], data[2]);
                }
                // PROBE #2763: each override-redirect window as it maps, and the pointer's
                // crossings of it.
                Event::MapNotify(map) if map.override_redirect => {
                    let events = ChangeWindowAttributesAux::new()
                        .event_mask(EventMask::ENTER_WINDOW | EventMask::LEAVE_WINDOW);
                    _ = watch.connection.change_window_attributes(map.window, &events).map(|c| c.check());
                    crate::log!("browser: probe map {:#x} override", map.window);
                }
                Event::UnmapNotify(unmap) => crate::log!("browser: probe unmap {:#x}", unmap.window),
                Event::EnterNotify(cross) => crate::log!("browser: probe enter {:#x} mode {:?} detail {:?} child {:#x} at {} {} time {}", cross.event, cross.mode, cross.detail, cross.child, cross.root_x, cross.root_y, cross.time),
                Event::LeaveNotify(cross) => crate::log!("browser: probe leave {:#x} mode {:?} detail {:?} child {:#x} at {} {} time {}", cross.event, cross.mode, cross.detail, cross.child, cross.root_x, cross.root_y, cross.time),
                Event::FocusIn(focus) => crate::log!("browser: probe focus in {:#x} mode {:?} detail {:?}, focus {}", focus.event, focus.mode, focus.detail, probe_focus(&watch)),
                Event::FocusOut(focus) => crate::log!("browser: probe focus out {:#x} mode {:?} detail {:?}", focus.event, focus.mode, focus.detail),
                _ => {}
            }
        }
        if !changed {
            return;
        }
        tell_all();
    }
}

/// The window `window` names in its `property`, or None.
fn window_in(watch: &Watch, window: xproto::Window, property: xproto::Atom) -> Option<u32> {
    let reply = watch
        .connection
        .get_property(false, window, property, AtomEnum::WINDOW, 0, 1)
        .ok()?
        .reply()
        .ok()?;
    reply.value32()?.next().filter(|&window| window != 0)
}

/// Whether the browser window GPUI's window `ui_view` is in is the active one, or the one the
/// active window is transient for.
fn is_active(watch: &Watch, ui_view: *mut c_void, active: Option<u32>) -> bool {
    let Some(active) = active else {
        return false;
    };
    let transient_for = window_in(watch, active, AtomEnum::WM_TRANSIENT_FOR.into());
    let parent = watch
        .connection
        .query_tree(ui_view as usize as u32)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .map(|tree| tree.parent);
    parent.is_some_and(|parent| parent == active || Some(parent) == transient_for)
}

/// Whether the keyboard is in a window the window manager doesn't manage, as an overlay's or a
/// menu's, which it then says no window is active for: the window under it is still the user's.
/// Another app's popup takes the keyboard only once its own window is active, which ends ours.
fn popup_has_focus(watch: &Watch) -> bool {
    let focus = watch
        .connection
        .get_input_focus()
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .map(|reply| reply.focus)
        .filter(|&focus| focus > 1);
    focus
        .and_then(|focus| watch.connection.get_window_attributes(focus).ok())
        .and_then(|cookie| cookie.reply().ok())
        .is_some_and(|attributes| attributes.override_redirect)
}

/// Tells each browser window whether it's the one the user is in, on X11.
fn tell_all() {
    let Some(watch) = WATCH.with_borrow(Clone::clone) else {
        return;
    };
    let views: Vec<*mut c_void> = WINDOWS.with(|windows| {
        windows
            .try_borrow()
            .map(|windows| windows.iter().map(|w| w.ui_view).collect())
            .unwrap_or_default()
    });
    let active = window_in(&watch, watch.root, watch.active);
    if active.is_none() && popup_has_focus(&watch) {
        return;
    }
    for ui_view in views {
        probe_select(&watch, ui_view);
        tell(ui_view, is_active(&watch, ui_view, active));
    }
}

fn tell(ui_view: *mut c_void, main: bool) {
    let told = TOLD.with_borrow_mut(|told| {
        match told.iter_mut().find(|(each, _)| *each == ui_view) {
            Some((_, last)) if *last == main => return false,
            Some((_, last)) => *last = main,
            None => told.push((ui_view, main)),
        }
        true
    });
    if told && let Some(changed) = CHANGED.with_borrow(|changed| *changed) {
        changed(ui_view, main);
    }
}

/// Forgets the window of `ui_view`, closed.
pub(crate) fn forget_window(ui_view: *mut c_void) {
    TOLD.with_borrow_mut(|told| told.retain(|(each, _)| *each != ui_view));
}

/// PROBE #2762: the input focus and the window it's in.
fn probe_focus(watch: &Watch) -> String {
    let focus = watch.connection.get_input_focus().ok().and_then(|c| c.reply().ok()).map_or(0, |r| r.focus);
    let parent = watch.connection.query_tree(focus).ok().and_then(|c| c.reply().ok()).map_or(0, |t| t.parent);
    format!("{focus:#x} in {parent:#x}")
}

/// PROBE #2762: hears each focus change on GPUI's window `ui_view` and the window it's in, once.
fn probe_select(watch: &Watch, ui_view: *mut c_void) {
    thread_local! { static SEEN: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) }; }
    let view = ui_view as usize as u32;
    if SEEN.with_borrow(|seen| seen.contains(&view)) {
        return;
    }
    SEEN.with_borrow_mut(|seen| seen.push(view));
    let parent = watch.connection.query_tree(view).ok().and_then(|c| c.reply().ok()).map_or(0, |t| t.parent);
    let events = ChangeWindowAttributesAux::new().event_mask(EventMask::FOCUS_CHANGE);
    for window in [view, parent] {
        _ = watch.connection.change_window_attributes(window, &events).map(|c| c.check());
    }
    let id = crate::browser_windows::window_with_view(ui_view);
    crate::log!("browser: probe window {id} ui {view:#x} in {parent:#x}");
}
