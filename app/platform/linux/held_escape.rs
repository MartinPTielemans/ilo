//! A held Escape's repeats reach only the window its press went to, as on the Mac (host.mm's
//! WatchHeldEscape). The command bar and the find bar close on Escape going down, which gives the
//! browser window back the keyboard, and the X server's repeats of the held key would go on to the
//! page there, taking it out of its own full screen or closing its dialog.
//!
//! An Escape press GPUI gets marks the key held, until this connection of the host's own hears it
//! let go. Until then the page's Escape presses are the held key's repeats, and the app keeps them.
//! A keyboard's release is heard as the screen's raw release: Chromium's browser window selects
//! XI2 keys, which the X server offers ahead of core ones and stops at (#2737). A release sent to
//! a window, as the checks' driver sends one, makes no raw event, so the core release is heard too,
//! in the browser windows and the window the press went to.
//!
//! A Wayland compositor sends no repeats: each client makes its own from the keys it got, and
//! every key reaches both Chromium and GPUI. So there the page's Escape presses are the held key's
//! while GPUI still repeats the press the UI got (`gpui_linux::held_key`), which a release or
//! another press ends.

use std::cell::{Cell, RefCell};

use gpui::App;
use x11rb::{
    connection::{Connection as _, RequestConnection as _},
    protocol::{
        Event,
        xinput::{self, ConnectionExt as _},
        xkb,
        xkb::ConnectionExt as _,
        xproto::{self, ChangeWindowAttributesAux, ConnectionExt as _, EventMask},
    },
    rust_connection::RustConnection,
};

use crate::{WINDOWS, bc};

/// Escape's keysym, which the X server's keyboard map gives a keycode.
const ESCAPE: xproto::Keysym = 0xff1b;

thread_local! {
    /// Whether the app is on Wayland, where GPUI says what's held.
    static WAYLAND: Cell<bool> = const { Cell::new(false) };
    /// Under Wayland, GPUI's number for the Escape press the UI got, while it may be held.
    static WAYLAND_PRESS: Cell<Option<u64>> = const { Cell::new(None) };
    static X: RefCell<Option<X>> = const { RefCell::new(None) };
    /// The windows that tell this connection their key releases while Escape is held.
    static HELD: RefCell<Option<Vec<xproto::Window>>> = const { RefCell::new(None) };
}

/// The host's own connection to the X server, its screen's root and Escape's keycode on it.
struct X {
    connection: RustConnection,
    root: xproto::Window,
    escape: xproto::Keycode,
}

/// Watches the UI's keys for Escape going down, on Wayland if `wayland`.
pub(crate) fn watch(wayland: bool, cx: &mut App) {
    WAYLAND.set(wayland);
    cx.intercept_keystrokes(move |event, _, _| {
        let keystroke = &event.keystroke;
        let escape = keystroke.key == "escape" && !keystroke.modifiers.modified();
        if wayland {
            WAYLAND_PRESS.set(escape.then(repeating_escape).flatten());
            return;
        }
        let_go();
        if escape {
            pressed();
        }
    })
    .detach();
}

/// GPUI's number for the press of the Escape it repeats, if it's Escape it's repeating.
fn repeating_escape() -> Option<u64> {
    gpui_linux::held_key().and_then(|(keysym, press)| (keysym == ESCAPE).then_some(press))
}

/// Whether `key`, going to a page, is the repeat of an Escape held since the UI got its press.
pub(crate) fn keeps_from_page(key: &bc::bc_key) -> bool {
    let escape = unsafe { std::ffi::CStr::from_ptr(key.key) }.to_bytes() == b"Escape";
    if WAYLAND.get() {
        let press = WAYLAND_PRESS.take();
        let held = escape && key.modifiers == 0 && press.is_some() && press == repeating_escape();
        WAYLAND_PRESS.set(press.filter(|_| held));
        return held;
    }
    if !escape || key.modifiers != 0 || HELD.with_borrow(Option::is_none) || released() {
        let_go();
        return false;
    }
    true
}

fn pressed() {
    let held = X.with_borrow_mut(|x| {
        if x.is_none() {
            *x = connect();
        }
        let X {
            connection, root, ..
        } = x.as_ref()?;
        raw_releases(connection, *root, true);
        let focus = connection.get_input_focus().ok()?.reply().ok()?.focus;
        let mut windows = vec![focus];
        // The browser windows, which the core's UI views are in, and the windows in them, which
        // the release goes to when the page or the UI has the keyboard.
        let views: Vec<u32> =
            WINDOWS.with_borrow(|all| all.iter().map(|w| w.ui_view as usize as u32).collect());
        for view in views {
            let tree = connection
                .query_tree(view)
                .ok()
                .and_then(|c| c.reply().ok());
            if let Some(tree) = tree {
                windows.push(tree.parent);
                within(connection, tree.parent, &mut windows);
            }
        }
        select(connection, &windows, EventMask::KEY_RELEASE);
        Some(windows)
    });
    HELD.set(held);
}

fn within(connection: &RustConnection, window: xproto::Window, windows: &mut Vec<xproto::Window>) {
    let Some(tree) = connection
        .query_tree(window)
        .ok()
        .and_then(|c| c.reply().ok())
    else {
        return;
    };
    for child in tree.children {
        windows.push(child);
        within(connection, child, windows);
    }
}

/// Whether the held Escape has been let go since it went down.
fn released() -> bool {
    X.with_borrow(|x| {
        let Some(X {
            connection, escape, ..
        }) = x
        else {
            return true;
        };
        let mut released = false;
        while let Ok(Some(event)) = connection.poll_for_event() {
            released |= match event {
                Event::KeyRelease(release) => release.detail == *escape,
                Event::XinputRawKeyRelease(release) => release.detail == u32::from(*escape),
                _ => false,
            };
        }
        released
    })
}

/// Escape's let go or another key's pressed: the windows stop telling this connection their
/// releases, and those it told are dropped.
fn let_go() {
    let Some(windows) = HELD.take() else { return };
    X.with_borrow(|x| {
        if let Some(X {
            connection, root, ..
        }) = x
        {
            raw_releases(connection, *root, false);
            select(connection, &windows, EventMask::NO_EVENT);
            while let Ok(Some(_)) = connection.poll_for_event() {}
        }
    });
}

fn select(connection: &RustConnection, windows: &[xproto::Window], mask: EventMask) {
    let aux = ChangeWindowAttributesAux::new().event_mask(mask);
    for &window in windows {
        // A window that has closed since answers with an error, which is dropped.
        if let Ok(cookie) = connection.change_window_attributes(window, &aux) {
            cookie.ignore_error();
        }
    }
    connection.flush().ok();
}

/// Whether the screen's raw key releases come to `connection`, from every keyboard whatever has
/// the focus, which takes them from no other client.
fn raw_releases(connection: &RustConnection, root: xproto::Window, on: bool) {
    let mask = xinput::EventMask {
        deviceid: xinput::Device::ALL_MASTER.into(),
        mask: vec![match on {
            true => xinput::XIEventMask::RAW_KEY_RELEASE,
            false => 0u32.into(),
        }],
    };
    if let Ok(cookie) = connection.xinput_xi_select_events(root, &[mask]) {
        cookie.ignore_error();
    }
}

/// The host's own connection to the X server.
fn connect() -> Option<X> {
    let (connection, screen) = x11rb::connect(None).ok()?;
    let root = connection.setup().roots.get(screen)?.root;
    // Raw events reach every client that selects them on the root from XI 2.2.
    connection
        .xinput_xi_query_version(2, 2)
        .ok()?
        .reply()
        .ok()?;
    // The X server sends a release before each repeat unless the client asks it not to, as
    // Chromium and GPUI do.
    connection
        .extension_information(xkb::X11_EXTENSION_NAME)
        .ok()??;
    connection.xkb_use_extension(1, 0).ok()?.reply().ok()?;
    let detectable = xkb::PerClientFlag::DETECTABLE_AUTO_REPEAT;
    let flags = connection.xkb_per_client_flags(
        xkb::ID::USE_CORE_KBD.into(),
        detectable,
        detectable,
        Default::default(),
        Default::default(),
        Default::default(),
    );
    flags.ok()?.reply().ok()?;
    let setup = connection.setup();
    let (first, count) = (setup.min_keycode, setup.max_keycode - setup.min_keycode + 1);
    let map = connection
        .get_keyboard_mapping(first, count)
        .ok()?
        .reply()
        .ok()?;
    let per = usize::from(map.keysyms_per_keycode);
    let index = map.keysyms.iter().position(|&keysym| keysym == ESCAPE)?;
    let escape = first + u8::try_from(index / per).ok()?;
    Some(X {
        connection,
        root,
        escape,
    })
}
