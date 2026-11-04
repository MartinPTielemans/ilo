//! The browser windows' urgency hint while tasks wait for the user (host.h's
//! host_needs_you_badge): Linux has no badge every desktop reads, so no count. On X11 it's the
//! urgency flag in each window's WM_HINTS, as Chromium's own FlashFrame sets it, on the browser
//! window that GPUI's is in. Wayland lets no client mark another's surface, Chromium's toplevel, so
//! there it shows nothing.

use std::ffi::c_int;

use x11rb::{
    connection::Connection as _,
    protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode},
    wrapper::ConnectionExt as _,
};

use crate::WINDOWS;

/// WM_HINTS' UrgencyHint, in its flags, the first of its nine words (ICCCM 4.1.2.4).
const URGENCY: u32 = 1 << 8;

#[unsafe(no_mangle)]
pub extern "C" fn host_needs_you_badge(count: c_int) {
    let urgent = count > 0;
    let views: Vec<_> =
        WINDOWS.with_borrow(|windows| windows.iter().map(|w| (w.id, w.ui_view)).collect());
    // Under Wayland the views are surfaces, which as X ids could name another client's windows.
    if crate::frames::on_x11() != Some(true) {
        return;
    }
    let Ok((connection, _)) = x11rb::connect(None) else {
        return;
    };
    for (id, view) in views {
        let Some(window) = (connection.query_tree(view as usize as u32).ok())
            .and_then(|tree| tree.reply().ok())
            .map(|tree| tree.parent)
        else {
            continue;
        };
        let hints = (connection
            .get_property(false, window, AtomEnum::WM_HINTS, AtomEnum::WM_HINTS, 0, 9)
            .ok())
        .and_then(|asked| asked.reply().ok());
        let mut words: Vec<u32> = hints
            .and_then(|hints| hints.value32().map(Iterator::collect))
            .unwrap_or_default();
        words.resize(9, 0);
        let flags = if urgent {
            words[0] | URGENCY
        } else {
            words[0] & !URGENCY
        };
        if flags == words[0] {
            continue;
        }
        words[0] = flags;
        let changed = connection.change_property32(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_HINTS,
            AtomEnum::WM_HINTS,
            &words,
        );
        if changed.is_ok() && connection.flush().is_ok() {
            // check.sh reads this line.
            browser::log!(
                "browser: window {id} urgent {}",
                if urgent { "on" } else { "off" }
            );
        }
    }
}

/// Whether ilo has the keyboard. On X11, whether the input focus is in a window of this process's,
/// as the X server says: GPUI hears no activation of its windows inside Chromium's there, so the
/// host's own tracking never moves. Under Wayland, the host's.
pub(crate) fn app_active() -> bool {
    if crate::frames::on_x11() != Some(true) {
        return unsafe { crate::platform::host_app_active() };
    }
    focus_is_ours().unwrap_or(false)
}

fn focus_is_ours() -> Option<bool> {
    let (connection, screen) = x11rb::connect(None).ok()?;
    let root = connection.setup().roots[screen].root;
    let pid = (connection
        .intern_atom(true, b"_NET_WM_PID")
        .ok()?
        .reply()
        .ok()?)
    .atom;
    let mut window = connection.get_input_focus().ok()?.reply().ok()?.focus;
    // 0 and 1 are None and PointerRoot: no window has it.
    while window > 1 && window != root {
        let owner = (connection
            .get_property(false, window, pid, AtomEnum::CARDINAL, 0, 1)
            .ok())
        .and_then(|asked| asked.reply().ok())
        .and_then(|owner| owner.value32().and_then(|mut values| values.next()));
        if let Some(owner) = owner {
            return Some(owner == std::process::id());
        }
        window = connection.query_tree(window).ok()?.reply().ok()?.parent;
    }
    Some(false)
}
