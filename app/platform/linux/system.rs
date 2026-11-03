//! What the Linux host does through GPUI's Linux platform rather than in host.cc, as GPUI already
//! has it: the clipboard, the file chooser portal, the browser window's activation, the touchpad's
//! swipes over the page, Control's release and a file's drag.

use std::{
    ffi::{CStr, c_char, c_void},
    path::PathBuf,
};

use browser::log;
use gpui::{App, ClipboardItem, Context, Image, ImageFormat, Window};

use crate::platform;

/// Puts `text` on the clipboard, as host_copy does on the Mac: a tab's URL, a link to it, a build's
/// details. check.sh's runs have a headless session's clipboard, never the user's.
pub fn copy(text: &str, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
}

/// The clipboard's text, as host_paste reads it on the Mac.
pub fn pasted(cx: &App) -> Option<String> {
    cx.read_from_clipboard()?.text()
}

/// Puts the capture on the clipboard, as host_copy_png does on the Mac.
pub fn copy_png(png: &[u8]) {
    let image = Image::from_bytes(ImageFormat::Png, png.to_vec());
    // After the update under way, which the capture's menu or toast is in.
    crate::update_app_later(move |cx| {
        cx.write_to_clipboard(ClipboardItem::new_image(&image));
        // check.sh reads this line, as the Mac's pasteboard one.
        log!("browser: clipboard png {} bytes", image.bytes.len());
    });
}

/// Asks where to save the capture, through the desktop's file chooser portal, and saves it there.
pub fn save_png(png: &[u8]) {
    let name = crate::capture_name();
    let png = png.to_vec();
    crate::update_app_later(move |cx| {
        // The portal opens in the folder given, and GPUI asks for one; the home folder is always
        // there.
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        let chosen = cx.prompt_for_new_path(&home, Some(&name));
        let background = cx.background_executor().clone();
        cx.foreground_executor()
            .spawn(async move {
                if let Ok(Ok(Some(path))) = chosen.await {
                    _ = background.spawn(async move { std::fs::write(path, png) }).await;
                }
            })
            .detach();
        // check.sh reads this line, as the Mac's.
        log!("browser: capture save panel shown");
    });
}

/// Tells the host each time the browser window of `ui_view`, whose UI `window` is, becomes active
/// or stops being, for its key window and active app watches. Under Wayland, `whole_window`, GPUI
/// has the keyboard the compositor gives the browser window.
pub fn watch_activation<T: 'static>(
    window: &mut Window,
    ui_view: *mut c_void,
    whole_window: bool,
    cx: &mut Context<T>,
) {
    cx.observe_window_activation(window, move |_, window, _| {
        let active = window.is_window_active();
        if active {
            crate::link_preview_window::took_keyboard(ui_view);
        }
        unsafe { platform::host_window_active(ui_view, active, whole_window) }
    })
    .detach();
}

/// Tells the link preview each time Settings, whose view is `view`, takes the keyboard.
pub fn watch_settings_activation<T: 'static>(
    window: &mut Window,
    view: *mut c_void,
    cx: &mut Context<T>,
) {
    cx.observe_window_activation(window, move |_, window, _| {
        if window.is_window_active() {
            crate::link_preview_window::took_keyboard(view);
        }
    })
    .detach();
}

/// Starts the system's drag of the file at `path`, as host.h says: XDND on X11, the data device's
/// drag under Wayland, from the press the left button holds.
#[unsafe(no_mangle)]
pub extern "C" fn host_drag_file(path: *const c_char, ended: extern "C" fn(taken: bool)) -> bool {
    let path = PathBuf::from(unsafe { CStr::from_ptr(path) }.to_string_lossy().into_owned());
    if !path.exists() {
        return false;
    }
    gpui_linux::start_file_drag(&path, move |taken| {
        // After the event that let it go, out of the display's handling.
        if let Some(cx) = crate::CX.with_borrow(Clone::clone) {
            cx.foreground_executor().spawn(async move { ended(taken) }).detach();
        }
    })
}

/// Calls `released` once Control is no longer held, after the event that shows it, as a ⌃Tab walk
/// ends: GPUI hears the keyboard's state whichever view has the keys.
#[unsafe(no_mangle)]
pub extern "C" fn host_watch_control(released: extern "C" fn()) {
    // After the update under way, which the walk's ⌃Tab asked from.
    gpui_linux::watch_control(move || crate::overlays::later(released));
}

/// Calls `tapped` with the UI's view of the browser window with the keyboard each time ⌥ is pressed
/// and let go alone, as host.h says. GPUI hears it in the UI; on X11 not in the page, whose keys
/// are Chromium's window's alone.
#[unsafe(no_mangle)]
pub extern "C" fn host_watch_option_taps(tapped: extern "C" fn(ui_view: *mut c_void)) {
    gpui_linux::watch_alt_taps(move || {
        let Some(cx) = crate::CX.with_borrow(Clone::clone) else { return };
        // After the event, out of the display's handling. Not in an overlay or Settings, which
        // are other windows.
        cx.spawn(async move |cx| {
            let active = cx.update(|cx| cx.active_window());
            let view = crate::WINDOWS.with_borrow(|windows| {
                let window = windows.iter().find(|window| Some(window.handle) == active)?;
                Some(window.ui_view)
            });
            if let Some(view) = view {
                tapped(view);
            }
        })
        .detach();
    });
}

/// A touchpad swipe over the page in the browser window whose Wayland surface is `surface`
/// (gpui_linux::watch_embedder_swipes).
pub fn swiped(surface: *mut c_void, x: f32, y: f32, back: bool) {
    crate::swiped(surface, x.into(), y.into(), back);
}
