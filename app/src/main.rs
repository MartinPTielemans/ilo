//! The browser. The core (core/, in Chrome's framework) makes the browser window and puts the page
//! in it, through its C API; GPUI draws the shell in a view inside that window, under the page.
//! Chromium owns the application object and the run loop.

// The Windows build names its crate Browser, for Browser.dll (windows/Cargo.toml).
#![cfg_attr(target_os = "windows", allow(non_snake_case))]

// The engine linked was built from another core/public/browser_core.h than the bindings (build.rs).
#[cfg(engine_header_mismatch)]
compile_error!(env!("ENGINE_HEADER_MISMATCH"));

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::{CStr, CString, c_char, c_int, c_void},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
use browser::flatpak;
use browser::window_frames::{self, Frame};
use browser::widevine::Widevine;
use browser::{
    account, capture, command_bar, dialog, dock_dot, elevation::Level, folder_preview, hover_card,
    icon_picker, launch, link_preview, link_target, log, menu, offer, passwords, site_controls,
    theme_picker, toast, tooltip, zoom_hud,
};
use browser::shell::{
    self, Archived, BarTab, Browser, Command, Download, DownloadAction, DownloadState, Extension,
    ExtensionAction, ExtensionCommand, FilePreview, Folders, MediaAction, MenuEntry, Page,
    PagePicture, Shell, SidePanel, Split, Trouble, WindowButton,
};
use browser::theme::Appearance;
use browser::text_input::{
    Copy, Cut, Paste, PasteAndMatchStyle, Redo, SelectAll, Undo,
};
use gpui::{
    AnyWindowHandle, App, ApplicationHandle, AsyncApp, Bounds, Context, Focusable, Image,
    ImageFormat, KeyBinding, Menu, MenuItem, OsAction, Pixels, Point, SharedString, Size,
    SystemMenuType, WeakEntity, Window, WindowBackgroundAppearance, WindowHandle, WindowOptions,
    actions, point, prelude::*, px, size,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

mod drop_targets;
#[cfg(target_os = "windows")]
#[path = "../platform/windows/drive_menus.rs"]
mod drive_menus;
#[cfg(target_os = "windows")]
#[path = "../platform/windows/fade.rs"]
mod fade;
mod find_bar;
mod import_host;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/drive.rs"]
mod drive;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/drive_wayland.rs"]
mod drive_wayland;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/drops.rs"]
mod drops;
#[cfg(any(target_os = "linux", target_os = "windows"))]
#[path = "../platform/linux/emoji.rs"]
mod emoji;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/floats.rs"]
mod floats;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/frames.rs"]
mod frames;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/held_escape.rs"]
mod held_escape;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/link_preview_window.rs"]
mod link_preview_window;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/menu_window.rs"]
mod menu_window;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/overlays.rs"]
mod overlays;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/panels.rs"]
mod panels;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/popovers.rs"]
mod popovers;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/previews.rs"]
mod previews;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/reduce_motion.rs"]
mod reduce_motion;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/reveal_window.rs"]
mod reveal_window;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/settings_window.rs"]
mod settings_window;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/motion.rs"]
mod motion;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/shadows.rs"]
mod shadows;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/system.rs"]
mod system;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/window_buttons.rs"]
mod window_buttons;
mod link_rules_host;
mod reveal;
mod agents_core;
mod settings;

/// The host's overlays (platform/host.h).
const COMMAND_BAR: c_int = 0;
const ICON_PICKER: c_int = 1;
const THEME_PICKER: c_int = 2;
const FOLDER_PREVIEW: c_int = 3;
/// The Site Controls aren't one of the host's overlays, but its elevation call takes them too.
const SITE_CONTROLS: c_int = 4;

use browser::bc;

/// The AppKit side, in platform/mac/, or its Windows counterpart in platform/windows/.
mod platform {
    use std::ffi::{c_char, c_int, c_void};

    unsafe extern "C" {
        #[cfg(not(target_os = "linux"))]
        pub fn drive_start();
        #[cfg(target_os = "windows")]
        pub fn drive_watch_menus(
            menus: extern "C" fn(ui_view: *mut c_void, title: *const c_char, bar: bool) -> bool,
        );
        #[cfg(target_os = "linux")]
        pub fn host_choose(title: *const c_char) -> bool;
        pub fn host_centered(width: f64, height: f64, x: *mut f64, y: *mut f64);
        #[cfg(not(target_os = "linux"))]
        pub fn host_ui_view() -> *mut c_void;
        #[cfg(target_os = "linux")]
        pub fn host_watch_fd(fd: c_int, ready: extern "C" fn());
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        pub fn host_window_opened(ui_view: *mut c_void, window: u64);
        #[cfg(target_os = "linux")]
        pub fn host_window_closed(ui_view: *mut c_void);
        #[cfg(target_os = "linux")]
        pub fn host_window_active(ui_view: *mut c_void, active: bool, whole_window: bool);
        pub fn host_capture_name(name: *mut c_char, capacity: usize);
        pub fn host_show_window(ui_view: *mut c_void);
        pub fn host_show_window_behind(ui_view: *mut c_void, above_ui_view: *mut c_void);
        pub fn host_show_launch_window(
            profile_dir: *const c_char,
            x: f64,
            y: f64,
            width: f64,
            height: f64,
        );
        pub fn host_screens(frames: *mut f64, count: usize) -> usize;
        pub fn host_window_frame(
            ui_view: *mut c_void,
            x: *mut f64,
            y: *mut f64,
            width: *mut f64,
            height: *mut f64,
        ) -> bool;
        pub fn host_close_launch_window();
        pub fn host_watch_finished_launching(finished: extern "C" fn());
        pub fn host_save_launch_picture(
            ui_view: *mut c_void,
            profile_dir: *const c_char,
            width: f64,
        ) -> bool;
        pub fn host_cascaded(
            ui_view: *mut c_void,
            x: *mut f64,
            y: *mut f64,
            width: *mut f64,
            height: *mut f64,
        );
        pub fn host_set_window_title(ui_view: *mut c_void, title: *const c_char);
        pub fn host_set_window_light(ui_view: *mut c_void, light: bool);
        pub fn host_set_app_appearance(appearance: c_int);
        pub fn host_activate(ui_view: *mut c_void);
        pub fn host_watch_key_window(changed: extern "C" fn(*mut c_void));
        pub fn host_watch_full_screen(changed: extern "C" fn(*mut c_void, bool));
        pub fn host_watch_swipes(swiped: extern "C" fn(*mut c_void, f64, f64, bool) -> bool);
        pub fn host_watch_option_taps(tapped: extern "C" fn(*mut c_void));
        pub fn host_after_menus(apply: extern "C" fn()) -> bool;
        pub fn host_watch_resigning_active(resigning: extern "C" fn());
        pub fn host_watch_active(became_active: extern "C" fn());
        #[cfg(not(target_os = "linux"))]
        pub fn host_reduce_motion() -> bool;
        #[cfg(not(target_os = "linux"))]
        pub fn host_watch_reduce_motion(changed: extern "C" fn());
        pub fn host_is_default_browser() -> bool;
        pub fn host_make_default_browser(answered: extern "C" fn());
        pub fn host_since_launch() -> f64;
        pub fn host_forget_window(ui_view: *mut c_void);
        pub fn host_add_overlay(
            overlay: c_int,
            overlay_view: *mut c_void,
            radius: f64,
            dismissed: extern "C" fn(),
        );
        pub fn host_set_overlay_elevation(
            overlay: c_int,
            outline_opacity: f32,
            outline_radius: f64,
            shadow_opacity: f32,
            shadow_radius: f64,
            shadow_drop: f64,
        );
        pub fn host_set_overlay_shape(overlay: c_int, corners: *const f64, count: usize);
        pub fn host_set_overlay_radius(overlay: c_int, radius: f64);
        pub fn host_set_overlay_growth(overlay: c_int, x: f64, y: f64);
        pub fn host_set_overlay_shrink(overlay: c_int);
        pub fn host_show_overlay(
            overlay: c_int,
            ui_view: *mut c_void,
            centered: bool,
            left: f64,
            top: f64,
            width: f64,
        );
        pub fn host_set_overlay_height(overlay: c_int, height: f64);
        pub fn host_hide_overlay(overlay: c_int);
        pub fn host_can_draw_emoji(emoji: *const c_char) -> bool;
        pub fn host_dismiss_overlay(overlay: c_int);
        #[cfg(target_os = "macos")]
        pub fn host_take_overlay_keyboard(overlay: c_int);
        pub fn host_hold_keys();
        pub fn host_release_held_keys();
        pub fn host_add_find_bar(
            ui_view: *mut c_void,
            find_bar_view: *mut c_void,
            right: f64,
            top: f64,
        );
        pub fn host_show_find_bar(ui_view: *mut c_void, keyboard: bool);
        pub fn host_hide_find_bar(ui_view: *mut c_void);
        pub fn host_add_offer(ui_view: *mut c_void, offer_view: *mut c_void, right: f64, top: f64);
        pub fn host_show_offer(ui_view: *mut c_void);
        pub fn host_hide_offer(ui_view: *mut c_void);
        pub fn host_add_hud(hud_view: *mut c_void, right: f64, top: f64, margin: f64);
        pub fn host_set_pane_corner(ui_view: *mut c_void, right: f64, top: f64);
        pub fn host_show_hud(ui_view: *mut c_void);
        pub fn host_hide_hud();
        pub fn host_add_drop_targets(view: *mut c_void);
        pub fn host_show_drop_targets(ui_view: *mut c_void);
        pub fn host_hide_drop_targets();
        pub fn host_focus(view: *mut c_void);
        pub fn host_show_window_buttons(ui_view: *mut c_void, shown: bool);
        pub fn host_press_window_button(ui_view: *mut c_void, button: i32);
        #[cfg(target_os = "macos")]
        pub fn host_window_buttons(
            ui_view: *mut c_void,
            centres: *mut f64,
            y: *mut f64,
            diameter: *mut f64,
        ) -> bool;
        #[cfg(target_os = "windows")]
        pub fn host_start_window_move(ui_view: *mut c_void);
        #[cfg(target_os = "windows")]
        pub fn host_window_maximized(ui_view: *mut c_void) -> bool;
        pub fn host_add_reveal(
            ui_view: *mut c_void,
            reveal_view: *mut c_void,
            pointer_moved: extern "C" fn(ui_view: *mut c_void, x: f64),
        );
        pub fn host_show_reveal(ui_view: *mut c_void, width: f64);
        pub fn host_hide_reveal(ui_view: *mut c_void);
        pub fn host_detent();
        pub fn host_dock_dot_launch(profile_dir: *const c_char);
        pub fn host_dock_dot(colored: bool, color: u32);
        pub fn host_watch_control(released: extern "C" fn());
        pub fn host_take_drops(
            dragged: extern "C" fn(
                ui_view: *mut c_void,
                phase: c_int,
                x: f64,
                y: f64,
                items: *const host_dragged,
                count: usize,
            ) -> bool,
        );
        pub fn host_trash(path: *const c_char) -> bool;
        pub fn host_drag_file(path: *const c_char, ended: extern "C" fn(taken: bool)) -> bool;
        pub fn host_file_preview(
            path: *const c_char,
            id: u64,
            done: extern "C" fn(
                id: u64,
                png: *const u8,
                size: usize,
                width: f64,
                height: f64,
                image: bool,
            ),
        );
        pub fn host_video_open(path: *const c_char, side: c_int) -> *mut c_void;
        pub fn host_video_frame(video: *mut c_void, pixels: *mut u8) -> bool;
        pub fn host_video_close(video: *mut c_void);
        #[cfg(not(target_os = "linux"))]
        pub fn host_copy(text: *const c_char);
        pub fn host_copy_secret(text: *const c_char, seconds: u32) -> bool;
        pub fn host_forget_secret();

        pub fn host_key_repeated() -> bool;
        #[cfg(not(target_os = "linux"))]
        pub fn host_paste(text: *mut c_char, capacity: usize) -> usize;
        pub fn host_open_in_safari(url: *const c_char);
        pub fn host_open_full_disk_access();
        pub fn host_share(
            ui_view: *mut c_void,
            x: f64,
            y: f64,
            width: f64,
            height: f64,
            url: *const c_char,
            title: *const c_char,
        );
        pub fn host_share_services(
            url: *const c_char,
            each: unsafe extern "C" fn(context: *mut c_void, name: *const c_char),
            context: *mut c_void,
        );
        pub fn host_share_with(url: *const c_char, title: *const c_char, index: usize);
        pub fn host_day_starts(starts: *mut f64);
        pub fn host_add_settings(settings_view: *mut c_void);
        pub fn host_show_settings();
        pub fn host_hide_settings();
        pub fn host_close_other_window() -> bool;
        #[cfg(not(target_os = "linux"))]
        pub fn host_ask(
            message: *const c_char,
            detail: *const c_char,
            buttons: *const *const c_char,
            count: usize,
            returns: bool,
        ) -> i32;
        pub fn host_add_dialog(dialog_view: *mut c_void);
        pub fn host_show_dialog(ui_view: *mut c_void, x: f64, y: f64, width: f64, height: f64);
        pub fn host_hide_dialog();
        pub fn host_add_site_controls(view: *mut c_void, dismissed: extern "C" fn());
        pub fn host_add_menu_window(
            view: *mut c_void,
            pointer: extern "C" fn(kind: i32, x: f64, y: f64) -> bool,
            dismissed: extern "C" fn(),
            choose: extern "C" fn(title: *const c_char) -> bool,
        );
        pub fn host_show_menu_window(
            ui_view: *mut c_void,
            in_reveal: bool,
            left: f64,
            top: f64,
            width: f64,
            height: f64,
        );
        pub fn host_hide_menu_window();
        pub fn host_menu_area(ui_view: *mut c_void, in_reveal: bool, area: *mut f64);
        pub fn host_show_site_controls(
            ui_view: *mut c_void,
            left: f64,
            top: f64,
            width: f64,
            height: f64,
        );
        pub fn host_set_site_controls_shape(corners: *const f64, count: usize);
        pub fn host_set_site_controls_height(height: f64);
        pub fn host_site_controls_drew(scale: f64, opacity: f64, x: f64, y: f64);
        pub fn host_release_site_controls();
        pub fn host_hide_site_controls();
        pub fn host_add_link_preview(
            view: *mut c_void,
            dismissed: extern "C" fn(),
        ) -> *mut c_void;
        pub fn host_show_link_preview(ui_view: *mut c_void);
        pub fn host_link_preview_drew(
            left: f64,
            top: f64,
            width: f64,
            height: f64,
            radius: f64,
            opacity: f64,
        );
        pub fn host_release_link_preview();
        pub fn host_hide_link_preview();
        pub fn host_add_capture(view: *mut c_void, dismissed: extern "C" fn());
        pub fn host_show_capture(
            ui_view: *mut c_void,
            left: f64,
            top: f64,
            width: f64,
            height: f64,
        );
        pub fn host_capture_camera(camera: bool);
        pub fn host_hide_capture();
        // GPUI's on Linux (platform/linux/system.rs).
        #[cfg(not(target_os = "linux"))]
        pub fn host_copy_png(png: *const u8, size: usize);
        #[cfg(not(target_os = "linux"))]
        pub fn host_save_png(ui_view: *mut c_void, png: *const u8, size: usize);
        pub fn host_share_png(
            png: *const u8,
            size: usize,
            view: *mut c_void,
            x: f64,
            y: f64,
            width: f64,
            height: f64,
            messages: bool,
        );
        pub fn host_add_toast(toast_view: *mut c_void);
        pub fn host_toast_takes_clicks(takes: bool);
        pub fn host_set_toast_plate(x: f64, y: f64, width: f64, height: f64);
        pub fn host_show_toast(ui_view: *mut c_void);
        pub fn host_hide_toast();
        pub fn host_add_tooltip(tooltip_view: *mut c_void, margin: f64, away: extern "C" fn());
        pub fn host_show_tooltip(ui_view: *mut c_void, left: f64, top: f64, fade: f64);
        pub fn host_hide_tooltip(fade: f64);
        pub fn host_add_hover_card(card_view: *mut c_void, margin: f64, away: extern "C" fn());
        pub fn host_show_hover_card(ui_view: *mut c_void, left: f64, top: f64, width: f64, height: f64);
        pub fn host_hide_hover_card();
        pub fn host_watch_memory_pressure(pressed: extern "C" fn());
        pub fn host_add_link_target(
            link_view: *mut c_void,
            margin: f64,
            pointer: extern "C" fn(f64, f64),
        );
        pub fn host_show_link_target(ui_view: *mut c_void, left: f64, top: f64, fade: f64);
        pub fn host_hide_link_target(fade: f64);
        #[cfg(target_os = "macos")]
        pub fn host_keep_keychain_key() -> bool;
        #[cfg(not(target_os = "linux"))]
        pub fn host_updates_available() -> bool;
        #[cfg(not(target_os = "linux"))]
        pub fn host_start_updater(quit: extern "C" fn());
        #[cfg(not(target_os = "linux"))]
        pub fn host_check_for_updates();
        #[cfg(target_os = "macos")]
        pub fn host_take_update_notes(notes: extern "C" fn(url: *const c_char));
    }

    /// What host_take_drops hears of a drag.
    pub const DRAG_OVER: c_int = 0;
    pub const DRAG_DROP: c_int = 2;

    #[repr(C)]
    pub struct host_dragged {
        pub url: *const c_char,
        pub file: bool,
    }
}

const WINDOW_SIZE: (f64, f64) = (1280., 800.);

/// Where Add Extension… goes, as Dia's does.
const WEB_STORE: &str = "https://chromewebstore.google.com/";

/// Chrome's Clear browsing data dialog, over its settings page, in the profile of the tab's Space.
const CLEAR_BROWSING_DATA: &str = "chrome://settings/clearBrowserData";

actions!(browser, [Hide, HideOthers, ShowAll, Minimize]);
actions!(browser, [CheckForUpdates]);

/// A browser window: the core's, with the view the UI draws in and the shell drawn there.
#[derive(Clone)]
struct BrowserWindow {
    id: bc::bc_id,
    ui_view: *mut c_void,
    shell: WeakEntity<Shell>,
    handle: AnyWindowHandle,
    /// Its own overlays' windows, such as the find bar's, which close with it.
    children: Vec<AnyWindowHandle>,
    /// A private window's: it shows a Space of its own, in an off-the-record session.
    private: bool,
    /// The user's Space it showed last, not an agent's, or 0, for a link from another app.
    users_space: bc::bc_id,
}

thread_local! {
    // The core calls back into the UI from Chromium's event handling, outside any GPUI update.
    static CX: RefCell<Option<AsyncApp>> = const { RefCell::new(None) };
    // GPUI starts inside Chromium's run loop, from a callback, and lives as long as the process.
    static APP: RefCell<Option<ApplicationHandle>> = const { RefCell::new(None) };
    // The most recently key first, as the core keeps its windows.
    static WINDOWS: RefCell<Vec<BrowserWindow>> = const { RefCell::new(Vec::new()) };
    // The links other apps sent before the first window had brought back the Spaces, which they
    // open over; None after.
    static LAUNCH_LINKS: RefCell<Option<Vec<String>>> = const { RefCell::new(Some(Vec::new())) };
    // The account as the core last said, which it says first as it starts, before GPUI has: the
    // first window hands it to Settings.
    static SYNC_STATE: RefCell<Option<account::State>> = const { RefCell::new(None) };
    // What the menu bar was last built from, so a change to one part rebuilds it with the others.
    static MENUS: RefCell<Menus> = RefCell::default();
    // Whether a menu bar built from MENUS waits for the menu open to close.
    static MENUS_HELD: Cell<bool> = const { Cell::new(false) };
}

/// What the menu bar lists besides what never changes: what the key window's shell says of it, and
/// the titles Undo and Redo have for what the key window would undo and redo.
#[derive(Default)]
struct Menus {
    bar: shell::MenuBar,
    undo: Option<String>,
    redo: Option<String>,
}

impl Menus {
    fn build(&self) -> Vec<Menu> {
        menus(&self.bar, self.undo.as_deref(), self.redo.as_deref())
    }
}

/// Puts up the menu bar built from MENUS, or, while a menu is open, once none is, as AppKit can
/// crash when the main menu is replaced under an open menu. What changes meanwhile goes in the one
/// rebuild.
fn show_menus(cx: &mut App) {
    if MENUS_HELD.get() {
        return;
    }
    if unsafe { platform::host_after_menus(menus_closed) } {
        // check.sh reads these lines.
        log!("browser: menu bar waits for the menu open");
        MENUS_HELD.set(true);
        return;
    }
    cx.set_menus(MENUS.with_borrow(Menus::build));
}

extern "C" fn menus_closed() {
    log!("browser: menu bar put up once the menus closed");
    MENUS_HELD.set(false);
    update_app(show_menus);
}

/// "Undo" or "Redo", with what it would undo or redo after it.
fn edit_title(verb: &str, what: Option<&str>) -> String {
    match what {
        Some(what) => format!("{verb} {what}"),
        None => verb.to_string(),
    }
}

/// The core's id and the UI's view of the window that the overlays every window shares show over,
/// such as the command bar: the browser window last key. 0 and null before one opens.
fn front() -> (bc::bc_id, *mut c_void) {
    WINDOWS.with_borrow(|windows| {
        windows.first().map_or((0, std::ptr::null_mut()), |window| (window.id, window.ui_view))
    })
}

/// A browser window has gone full screen or left it, once macOS's transition is over. Later, as
/// AppKit can say so from inside an update of ours, such as one that closes the window.
extern "C" fn full_screen_changed(ui_view: *mut c_void, entered: bool) {
    let id = window_with_view(ui_view);
    update_app_later(move |cx| {
        update_window_in(id, cx, |shell, cx| shell.set_full_screen(entered, cx));
    });
}

extern "C" fn swiped(ui_view: *mut c_void, x: f64, y: f64, back: bool) -> bool {
    let at = point(px(x as f32), px(y as f32));
    update_window(window_with_view(ui_view), |shell, cx| shell.swiped(at, back, cx))
        .unwrap_or(false)
}

/// ⌥ was pressed and let go alone in the window whose UI draws in `ui_view`.
extern "C" fn option_tapped(ui_view: *mut c_void) {
    update_window(window_with_view(ui_view), |shell, cx| shell.option_tapped(cx));
}

/// The window whose UI draws in `ui_view`, or 0.
fn window_with_view(ui_view: *mut c_void) -> bc::bc_id {
    WINDOWS.with_borrow(|windows| {
        windows.iter().find(|window| window.ui_view == ui_view).map_or(0, |window| window.id)
    })
}

/// The shell drawn in `ui_view`, for an overlay to read the look of the window it shows over.
pub(crate) fn shell_with_view(ui_view: *mut c_void) -> WeakEntity<Shell> {
    WINDOWS.with_borrow(|windows| {
        windows
            .iter()
            .find(|window| window.ui_view == ui_view)
            .map_or_else(WeakEntity::new_invalid, |window| window.shell.clone())
    })
}

/// A browser window became key: it goes first, so the shared overlays and the menu bar follow it.
/// AppKit makes one key as another window orders out or closes, from inside the update that did
/// it, such as Settings' close (#819), so the shell hears it once that update is over.
extern "C" fn key_window_changed(ui_view: *mut c_void) {
    let moved = WINDOWS.with_borrow_mut(|windows| {
        let index = windows.iter().position(|window| window.ui_view == ui_view)?;
        (index > 0).then(|| {
            let window = windows.remove(index);
            windows.insert(0, window.clone());
            window.id
        })
    });
    if let Some(id) = moved {
        log!("browser: key window {id}");
        update_app_later(move |cx| {
            update_window_in(id, cx, |shell, cx| shell.became_key(cx));
        });
    }
}

/// Changes every window's shell, from a core callback about what they all show, such as the
/// Spaces and their tabs.
fn update_shells(change: impl Fn(&mut Shell, &mut Context<Shell>)) {
    let Some(cx) = CX.with_borrow(Clone::clone) else {
        return;
    };
    cx.update(|cx| update_shells_in(cx, change));
}

/// Changes every window's shell, as `update_shells`, in an update under way.
fn update_shells_in(cx: &mut App, change: impl Fn(&mut Shell, &mut Context<Shell>)) {
    let shells: Vec<_> =
        WINDOWS.with_borrow(|windows| windows.iter().map(|window| window.shell.clone()).collect());
    for shell in shells {
        shell.update(cx, &change).ok();
    }
}

/// Changes the shell of the window `id`, from a core callback about that window, and returns what
/// the change does, unless there's no such window.
fn update_window<R>(
    id: bc::bc_id,
    change: impl FnOnce(&mut Shell, &mut Context<Shell>) -> R,
) -> Option<R> {
    let shell = WINDOWS.with_borrow(|windows| {
        windows.iter().find(|window| window.id == id).map(|window| window.shell.clone())
    })?;
    let cx = CX.with_borrow(Clone::clone)?;
    cx.update(|cx| shell.update(cx, change).ok())
}

/// Changes the shell of the window `id`, as `update_window`, in an update under way.
fn update_window_in<R>(
    id: bc::bc_id,
    cx: &mut App,
    change: impl FnOnce(&mut Shell, &mut Context<Shell>) -> R,
) -> Option<R> {
    let shell = WINDOWS.with_borrow(|windows| {
        windows.iter().find(|window| window.id == id).map(|window| window.shell.clone())
    })?;
    shell.update(cx, change).ok()
}

/// The window showing `tab`'s page, as the core says, else the one whose shell thinks it does,
/// else the one last key, such as for a key from a page no shell has heard is shown yet.
/// Removes the profile's `login`, or takes its site off Never, as the Site Controls and Settings
/// do. The user's alone: agents get sessions, never secrets.
fn remove_login(profile: bc::bc_id, login: &passwords::Login) {
    let text = |text: &str| CString::new(text).unwrap_or_default();
    let (key, username) = (text(&login.key), text(&login.username));
    // check.sh reads this line.
    log!("browser: remove login {profile} {}", passwords::describe(std::slice::from_ref(login)));
    unsafe { bc::bc_profile_remove_login(profile, key.as_ptr(), username.as_ptr(), login.never) };
}

/// Saves the Spaces' favorites and pinned tabs as a Markdown file of links where the user picks,
/// for Settings' Export…. The user's alone, as Settings is.
fn export_spaces(cx: &mut App) {
    if let Some(shell) = shell_with_view(front().1).upgrade() {
        shell.update(cx, |shell, cx| shell.export_spaces(cx));
    }
}

fn tab_window(tab: u64, cx: &App) -> Option<BrowserWindow> {
    let core = unsafe { bc::bc_tab_window(tab) };
    WINDOWS.with_borrow(|windows| {
        let shows = |window: &&BrowserWindow| {
            window.shell.upgrade().is_some_and(|shell| shell.read(cx).shows(tab))
        };
        windows
            .iter()
            .find(|window| window.id == core)
            .or_else(|| windows.iter().find(shows))
            .or(windows.first())
            .cloned()
    })
}

/// Changes the shell of the window showing `tab`'s page (`tab_window`), from a core callback about
/// the page, and returns what the change does.
fn update_tab_window<R>(
    tab: u64,
    change: impl FnOnce(&mut Shell, &mut Context<Shell>) -> R,
) -> Option<R> {
    let cx = CX.with_borrow(Clone::clone)?;
    cx.update(|cx| tab_window(tab, cx)?.shell.update(cx, change).ok())
}

/// Changes the UI outside the shells, from a core callback.
fn update_app(change: impl FnOnce(&mut App)) {
    if let Some(cx) = CX.with_borrow(Clone::clone) {
        cx.update(change);
    }
}

/// Changes the app once the update under way, if any, is over, for a core callback that can come
/// from inside the call that asked for it.
fn update_app_later(change: impl FnOnce(&mut App) + 'static) {
    if let Some(cx) = CX.with_borrow(Clone::clone) {
        let executor = cx.foreground_executor().clone();
        executor.spawn(async move { cx.update(change) }).detach();
    }
}

/// The view of a window the host fades, as the tooltip's: on Windows the app paints its fades.
fn fading_view(window: &mut Window) -> *mut c_void {
    let view = ns_view(window);
    #[cfg(target_os = "windows")]
    fade::attach(window, view);
    view
}

/// The window's NSView, for the host.
fn ns_view(window: &Window) -> *mut c_void {
    match HasWindowHandle::window_handle(window).map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::AppKit(handle)) => handle.ns_view.as_ptr(),
        Ok(RawWindowHandle::Win32(handle)) => handle.hwnd.get() as *mut c_void,
        Ok(RawWindowHandle::Xcb(handle)) => handle.window.get() as usize as *mut c_void,
        Ok(RawWindowHandle::Wayland(handle)) => handle.surface.as_ptr(),
        _ => panic!("the window has no native view"),
    }
}

/// Shows ilo's menu of `items` at `at` in the content of `ui_view`'s window, any of the app's, or
/// of its reveal's panel if `in_reveal`, in `look`, and calls `chosen` once it closes.
fn show_menu(
    ui_view: *mut c_void,
    items: Vec<MenuEntry>,
    at: Point<Pixels>,
    in_reveal: bool,
    look: menu::Look,
    chosen: Box<dyn FnOnce(Option<usize>, &mut App)>,
    cx: &mut App,
) {
    with_overlays(
        move |cx| {
            MENU_OVER.set((ui_view, in_reveal));
            let mut area = [0f64; 4];
            unsafe { platform::host_menu_area(ui_view, in_reveal, area.as_mut_ptr()) };
            let [left, top, right, bottom] = area.map(|edge| px(edge as f32));
            let area = Bounds::from_corners(point(left, top), point(right, bottom));
            menu::show(menu::Request { items, at, area, look, chosen }, cx);
        },
        cx,
    );
}

/// Tells the core where the window's page goes and the radius of its corners
/// (`Shell::page_frame`).
fn set_page_frame(window: bc::bc_id, (frame, radius): (Bounds<Pixels>, f32)) {
    unsafe { bc::bc_window_set_page_frame(window, rect(frame), f64::from(radius)) };
}

/// Tells the core where the window's page, at `page`, and its side panel's page go, in a window
/// whose content is `viewport`, as it resizes or a page goes full screen between the shell's draws.
fn set_frames(
    window: bc::bc_id,
    shell: &Shell,
    page: (Bounds<Pixels>, f32),
    viewport: Size<Pixels>,
) {
    set_page_frame(window, page);
    let panel = shell.side_panel_page(viewport).unwrap_or_default();
    unsafe { bc::bc_window_set_side_panel_frame(window, rect(panel)) };
}

/// Whether host_share opens a share sheet, which the tab menu's Share… and Site Controls' Share
/// tile both ask. Linux's host has none yet.
fn has_share_sheet() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}

/// Opens the system's share sheet in `window` for the page at `url`, hanging from `frame`.
fn share_page(window: &BrowserWindow, url: &str, title: &str, frame: Bounds<Pixels>) {
    let (Ok(url), Ok(title)) = (CString::new(url), CString::new(title)) else {
        return;
    };
    let frame = rect(frame);
    let (x, y, width, height) = (frame.x, frame.y, frame.width, frame.height);
    let (url, title) = (url.as_ptr(), title.as_ptr());
    unsafe { platform::host_share(window.ui_view, x, y, width, height, url, title) };
}

fn rect(bounds: Bounds<Pixels>) -> bc::bc_rect {
    bc::bc_rect {
        x: f64::from(bounds.origin.x),
        y: f64::from(bounds.origin.y),
        width: f64::from(bounds.size.width),
        height: f64::from(bounds.size.height),
    }
}

/// Opens a tab loading `url` at the top of the Today tabs of the Space `window` shows, and shows
/// it there.
fn open_tab(window: bc::bc_id, url: &str) {
    let tab = open_tab_behind(window, url, false);
    if tab != 0 {
        unsafe { bc::bc_window_select(window, tab) };
    }
}

/// Opens a tab loading `url` at the top of the Today tabs of the Space `window` shows, behind the
/// one shown, or with `when_shown` loading it only once it's first shown. Returns it, or 0.
fn open_tab_behind(window: bc::bc_id, url: &str, when_shown: bool) -> u64 {
    match CString::new(url) {
        Ok(url) => unsafe { bc::bc_tab_open(window, url.as_ptr(), when_shown) },
        Err(_) => 0,
    }
}

/// Puts `tab` in a split view with `with`, on its right, and shows it in `window`. The pane being
/// added for it, if there is one, gives the split its room first, so it shows at once at its place.
/// ⌘Z takes it back out, or closes it if it was `opened` for the split.
fn split_tab(window: bc::bc_id, tab: u64, with: u64, opened: bool, cx: &mut App) {
    shell::pane_added(tab, with, opened, cx);
    unsafe {
        bc::bc_tab_split_with(tab, with, false);
        bc::bc_window_select(window, tab);
    }
}

/// `tab`, opened to join a split view in `window`, joined it: that window's ⌘Z closes it.
fn split_opened(window: bc::bc_id, tab: u64, cx: &mut App) {
    let shell = WINDOWS.with_borrow(|windows| {
        windows.iter().find(|each| each.id == window).map(|each| each.shell.clone())
    });
    if let Some(shell) = shell {
        shell.update(cx, |shell, cx| shell.split_opened(tab, cx)).ok();
    }
}

/// Opens a tab loading `url` in a split view with `with`, on its right, and shows it in `window`.
fn open_split(window: bc::bc_id, url: &str, with: u64, cx: &mut App) {
    let Ok(url) = CString::new(url) else {
        return;
    };
    let tab = unsafe { bc::bc_tab_open(window, url.as_ptr(), false) };
    if tab != 0 {
        split_tab(window, tab, with, true, cx);
    }
}

/// Makes a Space right after `after`, else after the others.
fn create_space(name: &str, after: bc::bc_id) -> bc::bc_id {
    let name = CString::new(name).unwrap_or_default();
    unsafe { bc::bc_space_create(name.as_ptr(), after) }
}

/// The URL the core's `get` copies for `text`.
fn url_for(text: &str, get: impl Fn(*const c_char, *mut c_char, usize) -> usize) -> Option<String> {
    let text = CString::new(text).ok()?;
    // Long enough for most, so the core reads what was typed once.
    let mut url = vec![0u8; 2048];
    let length = get(text.as_ptr(), url.as_mut_ptr().cast(), url.len());
    if length >= url.len() {
        url.resize(length + 1, 0);
        get(text.as_ptr(), url.as_mut_ptr().cast(), url.len());
    }
    url.truncate(length);
    (length > 0).then(|| String::from_utf8_lossy(&url).into_owned())
}

/// The dialog's requests, to the host's dialog window.
struct DialogHost;

impl dialog::Host for DialogHost {
    fn show(&self, over: Option<Bounds<Pixels>>) {
        let over = rect(over.unwrap_or_default());
        unsafe {
            platform::host_show_dialog(front().1, over.x, over.y, over.width, over.height)
        };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_dialog() };
    }
}

/// The Dock icon's dot, swapped by the host (platform/mac/dock.mm).
struct DockDot;

impl dock_dot::Host for DockDot {
    fn show(&self, color: Option<u32>) {
        unsafe { platform::host_dock_dot(color.is_some(), color.unwrap_or(0)) };
    }
}

/// The Site Controls' requests, as calls to the core and to the host's window for them, over the
/// window in front (`front`).
struct SiteControlsHost {
    /// The panel's view, for its menus, once its window is open.
    view: Cell<*mut c_void>,
}

/// The panel's shape, for the host's blur and shadow, as x, y and radius each.
fn set_site_controls_shape(corners: &[(f32, f32, f32)]) {
    let flat: Vec<f64> = (corners.iter())
        .flat_map(|&(x, y, radius)| [f64::from(x), f64::from(y), f64::from(radius)])
        .collect();
    unsafe { platform::host_set_site_controls_shape(flat.as_ptr(), corners.len()) };
}

impl site_controls::Host for SiteControlsHost {
    fn show(&self, at: Point<Pixels>, size: gpui::Size<Pixels>, corners: &[(f32, f32, f32)]) {
        set_site_controls_shape(corners);
        let (left, top) = (f64::from(at.x), f64::from(at.y));
        let (width, height) = (f64::from(size.width), f64::from(size.height));
        unsafe { platform::host_show_site_controls(front().1, left, top, width, height) };
    }

    fn set_height(&self, height: f32, corners: &[(f32, f32, f32)]) {
        set_site_controls_shape(corners);
        unsafe { platform::host_set_site_controls_height(f64::from(height)) };
    }

    fn drew(&self, scale: f32, opacity: f32, origin: Point<Pixels>) {
        let (x, y) = (f64::from(origin.x), f64::from(origin.y));
        unsafe { platform::host_site_controls_drew(scale.into(), opacity.into(), x, y) };
    }

    fn release(&self) {
        unsafe { platform::host_release_site_controls() };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_site_controls() };
    }

    fn read(&self, tab: u64) {
        unsafe { bc::bc_tab_site_controls(tab) };
    }

    fn stop_reading(&self) {
        unsafe { bc::bc_site_controls_close() };
    }

    fn set_permission(&self, tab: u64, kind: i32, value: site_controls::Value) {
        let value = match value {
            site_controls::Value::Allow => bc::BC_SITE_ALLOW,
            site_controls::Value::Block => bc::BC_SITE_BLOCK,
            site_controls::Value::Ask => bc::BC_SITE_ASK,
        };
        unsafe { bc::bc_tab_set_site_permission(tab, kind, value as i32) };
    }

    fn reset_permissions(&self, tab: u64) {
        unsafe { bc::bc_tab_reset_site_permissions(tab) };
    }

    fn set_system_engine(&self, tab: u64, on: bool) {
        match on {
            // As the protected-video note's button does.
            true => use_system_engine(tab),
            false => {
                unsafe { bc::bc_tab_set_system_engine(tab, false) };
            }
        }
    }

    fn allow_saving(&self, tab: u64, login: &passwords::Login) {
        remove_login(unsafe { bc::bc_tab_profile(tab) }, login);
    }

    fn set_developer_mode(&self, tab: u64, on: bool) {
        unsafe { bc::bc_tab_set_developer_mode(tab, on) };
    }

    fn set_ad_blocking(&self, tab: u64, on: bool) {
        unsafe { bc::bc_tab_set_ad_blocking(tab, on) };
        // Settings lists the sites it's off on.
        update_app_later(settings::refresh);
    }

    fn run(&self, run: site_controls::Run, tab: u64, cx: &mut App) {
        let Some(window) = tab_window(tab, cx) else {
            return;
        };
        match run {
            // The shell's commands, which the keys and the command bar run too.
            site_controls::Run::Command(command) => {
                window.shell.update(cx, |shell, cx| shell.run(command, "site-controls", cx)).ok();
            }
            site_controls::Run::Share(frame) => {
                let Some((url, title)) = window.shell.upgrade().map(|shell| {
                    let shell = shell.read(cx);
                    (shell.page(tab).url, shell.title(tab))
                }) else {
                    return;
                };
                share_page(&window, &url, &title, frame);
            }
            site_controls::Run::SiteSettings => {
                let url = window.shell.upgrade().map(|shell| shell.read(cx).page(tab).url);
                let settings = url.and_then(|url| site_controls::site_settings_url(&url));
                open_tab(window.id, settings.as_deref().unwrap_or("chrome://settings/content"));
            }
            site_controls::Run::Extension(id, tile) => {
                window.shell.update(cx, |shell, cx| shell.run_extension_from(&id, tile, cx)).ok();
            }
            site_controls::Run::Certificate => unsafe { bc::bc_tab_show_certificate(tab) },
            site_controls::Run::RemoveLogin(login) => {
                let profile = unsafe { bc::bc_tab_profile(tab) };
                let (message, detail) = passwords::question(&login);
                let confirmed = Box::new(move |_: &mut App| remove_login(profile, &login));
                dialog::ask(message, detail, "Remove", confirmed, cx);
            }
            site_controls::Run::Passwords => {
                let profile = unsafe { bc::bc_tab_profile(tab) };
                settings::show_passwords(Some(profile).filter(|&profile| profile != 0), cx);
            }
            site_controls::Run::ExtensionItem(id, item) => {
                window.shell.update(cx, |shell, cx| shell.extension_item(&id, item, cx)).ok();
            }
        }
    }

    fn menu(
        &self,
        items: Vec<MenuEntry>,
        position: Point<Pixels>,
        chosen: Box<dyn FnOnce(Option<usize>, &mut App)>,
        cx: &mut App,
    ) {
        // The panel's menus are dark, as the panel is.
        show_menu(self.view.get(), items, position, false, menu::Look::dark(), chosen, cx);
    }

    fn tooltip(&self, tip: Option<(SharedString, tooltip::Anchor)>, cx: &mut App) {
        match tip {
            Some((text, anchor)) => tooltip::hover(tooltip::Tip::named(text), anchor, cx),
            None => tooltip::dismiss(cx),
        }
    }

    fn has_share_sheet(&self) -> bool {
        has_share_sheet()
    }
}

thread_local! {
    /// The UI's view of the browser window ilo's menu shows over, and whether in its reveal's panel.
    static MENU_OVER: Cell<(*mut c_void, bool)> = const { Cell::new((std::ptr::null_mut(), false)) };
}

/// ilo's menu's requests, to the host's window for it.
struct MenuHost;

impl menu::Host for MenuHost {
    fn show(&self, frame: Bounds<Pixels>, panels: &[Bounds<Pixels>]) {
        // Linux's window takes the pointer on the panels alone, as its host can shape it.
        #[cfg(target_os = "linux")]
        menu_window::take_pointer_on(panels);
        #[cfg(not(target_os = "linux"))]
        let _ = panels;
        let (ui_view, in_reveal) = MENU_OVER.get();
        let (left, top) = (f64::from(frame.origin.x), f64::from(frame.origin.y));
        let (width, height) = (f64::from(frame.size.width), f64::from(frame.size.height));
        unsafe { platform::host_show_menu_window(ui_view, in_reveal, left, top, width, height) };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_menu_window() };
    }
}

/// A pointer event anywhere in the app while ilo's menu shows, at `x`, `y` from its window's top
/// left. Returns whether the menu takes it.
extern "C" fn menu_pointer(kind: i32, x: f64, y: f64) -> bool {
    let event = match kind {
        0 => menu::Pointer::Moved,
        1 => menu::Pointer::Pressed,
        2 => menu::Pointer::RightPressed,
        3 => menu::Pointer::Released,
        _ => menu::Pointer::Scrolled,
    };
    let mut taken = false;
    update_app(|cx| taken = menu::pointer(event, point(px(x as f32), px(y as f32)), cx));
    taken
}

/// The user left ilo's menu: another window or app, or its browser window changing.
extern "C" fn menu_dismissed() {
    update_app(menu::dismissed);
}

/// The driver chooses `title` in ilo's menu.
extern "C" fn menu_choose(title: *const c_char) -> bool {
    let title = unsafe { CStr::from_ptr(title) }.to_string_lossy().into_owned();
    let mut chosen = false;
    update_app(|cx| chosen = menu::choose(&title, cx));
    chosen
}

/// The user left the Site Controls: a click outside them, another window or another app.
extern "C" fn site_controls_dismissed() {
    update_app(site_controls::dismissed);
}

/// The link preview's requests, to the host's preview window and the core.
struct LinkPreviewHost {
    /// The view the host puts the preview's page in.
    holder: Cell<*mut c_void>,
    /// The browser window it shows over, until that window's page has the keyboard back.
    over: Cell<u64>,
}

impl link_preview::Host for LinkPreviewHost {
    fn place(&self, tab: u64, page: Bounds<Pixels>, radius: f32) {
        unsafe { bc::bc_tab_show_in(tab, self.holder.get(), rect(page), f64::from(radius)) };
    }

    fn show(&self, window: u64) {
        let ui_view = WINDOWS.with_borrow(|windows| {
            windows.iter().find(|each| each.id == window).map(|each| each.ui_view)
        });
        if let Some(ui_view) = ui_view {
            self.over.set(window);
            unsafe { platform::host_show_link_preview(ui_view) };
        }
    }

    fn drew(&self, page: Bounds<Pixels>, radius: f32, opacity: f32) {
        let (left, top) = (f64::from(page.origin.x), f64::from(page.origin.y));
        let (width, height) = (f64::from(page.size.width), f64::from(page.size.height));
        let (radius, opacity) = (f64::from(radius), f64::from(opacity));
        unsafe { platform::host_link_preview_drew(left, top, width, height, radius, opacity) };
    }

    fn page_under(&self) -> bool {
        // No window of the app's holds a page on Linux (link_preview_window.rs) or Windows
        // (platform/windows/host.cc).
        cfg!(any(target_os = "linux", target_os = "windows"))
    }

    fn release(&self) {
        unsafe { platform::host_release_link_preview() };
        // The window's page has the keyboard back from the preview's, which goes, or no page has
        // it, unless the user left for another window. Linux's host gives it as it releases;
        // Windows' can't name the window.
        let window = self.over.replace(0);
        if cfg!(target_os = "windows") && window != 0 && !LINK_PREVIEW_LEFT.get() {
            unsafe { bc::bc_window_focus_page(window) };
        }
    }

    fn hide(&self) {
        // Released as it left, unless it goes at once.
        self.release();
        unsafe { platform::host_hide_link_preview() };
    }

    fn close(&self, tab: u64) {
        unsafe { bc::bc_tab_close(tab, false) };
    }

    fn keep(&self, window: u64, tab: u64) {
        unsafe { bc::bc_window_keep_preview(window, tab) };
    }

    fn closed(&self, window: u64, closed: link_preview::Closed, undone: bool, cx: &mut App) {
        let shell = WINDOWS.with_borrow(|windows| {
            windows.iter().find(|each| each.id == window).map(|each| each.shell.clone())
        });
        if let Some(shell) = shell {
            shell.update(cx, |shell, cx| shell.preview_closed(closed, undone, cx)).ok();
        }
    }
}

extern "C" fn link_preview_dismissed() {
    LINK_PREVIEW_LEFT.set(true);
    update_app(link_preview::dismissed);
    LINK_PREVIEW_LEFT.set(false);
}

thread_local! {
    /// Whether the host has a view for a link preview's page.
    static LINK_PREVIEWS: Cell<bool> = const { Cell::new(false) };
    /// The user left the preview for another window, which keeps the keyboard.
    static LINK_PREVIEW_LEFT: Cell<bool> = const { Cell::new(false) };
}

/// A link is dragged over `tab`'s page: the window showing it shows the split view's targets, and
/// takes the drag while it's on one, opening the link there when it's let go.
unsafe extern "C" fn tab_link_dragged(
    _: *mut c_void,
    tab: bc::bc_id,
    phase: c_int,
    url: *const c_char,
    x: f64,
    y: f64,
) -> bool {
    let Some(cx) = CX.with_borrow(Clone::clone) else {
        return false;
    };
    let url = unsafe { CStr::from_ptr(url) }.to_string_lossy().into_owned();
    let point = point(px(x as f32), px(y as f32));
    let Some(window) = cx.update(|cx| tab_window(tab, cx)) else {
        return false;
    };
    let answer = update_window(window.id, |shell, cx| match phase as u32 {
        bc::BC_LINK_DRAG_OVER => shell.link_over_page(tab, point, &url, cx),
        bc::BC_LINK_DRAG_DROP => shell.link_dropped(tab, point, cx),
        _ => {
            shell.link_left_page(cx);
            false
        }
    });
    answer.unwrap_or(false)
}

/// A link clicked in `tab`'s page leads to another site. A pinned tab's or a favorite's opens in
/// the link preview, over the window showing the tab, unless the user turned previews off or a link
/// rule sends it to another Space.
unsafe extern "C" fn tab_link_to_site(
    _: *mut c_void,
    tab: bc::bc_id,
    url: *const c_char,
    x: f64,
    y: f64,
) -> bool {
    if !LINK_PREVIEWS.get() || !unsafe { bc::bc_link_previews() } {
        return false;
    }
    let Some(cx) = CX.with_borrow(Clone::clone) else {
        return false;
    };
    let over = cx.update(|cx| {
        let window = tab_window(tab, cx)?;
        let shell = window.shell.upgrade()?;
        let shell = shell.read(cx);
        let page = shell.page_laid().filter(|_| shell.shows(tab) && shell.previews_links_of(tab));
        Some((window.id, window.private, page?))
    });
    let Some((window, private, page)) = over else {
        return false;
    };
    let url = unsafe { CStr::from_ptr(url) }.to_owned();
    // A call's link opens as a tab of the Space shown, the one used last, as it outlasts a look.
    if link_preview::is_meeting(&url.to_string_lossy()) {
        // check.sh waits for this line.
        log!("browser: meeting link from {tab} opens as a tab");
        update_app_later(move |_| open_tab(window, &url.to_string_lossy()));
        return true;
    }
    // A link rule sends it to its Space instead, as one from another app, when that's another than
    // the one shown. Never from a private window, whose links stay off the record.
    let shown = unsafe { bc::bc_window_space(window) };
    let routed = match private {
        true => 0,
        false => link_rules_host::preview_route(&url.to_string_lossy(), shown),
    };
    if routed != 0 {
        update_app_later(move |_| {
            let url = url.to_string_lossy();
            let (to, opened, space) = open_link_in(window, &url, routed);
            // check.sh reads this line.
            log!("browser: link from {tab} {url}: tab {opened} in space {space} of window {to}");
            let ui_view = WINDOWS.with_borrow(|windows| {
                windows.iter().find(|w| w.id == to).map(|w| w.ui_view)
            });
            if let Some(ui_view) = ui_view.filter(|_| to != window) {
                unsafe { platform::host_activate(ui_view) };
            }
        });
        return true;
    }
    // Outside the navigation that asked, which the core cancels once this returns.
    update_app_later(move |cx| {
        let preview = unsafe { bc::bc_tab_open_preview(tab, url.as_ptr()) };
        if preview == 0 {
            log!("browser: link preview from {tab} not opened");
            return;
        }
        let link = page.0.origin + point(px(x as f32), px(y as f32));
        let url = url.to_string_lossy().into_owned().into();
        link_preview::show(window, preview, tab, url, page, link, cx);
    });
    true
}

/// A key from the link preview's page acts on the preview alone: done, and whether the page loses
/// it. None for another page's.
fn link_preview_key(tab: bc::bc_id, key: *const bc::bc_key, reserved: bool) -> Option<bool> {
    let window = unsafe { bc::bc_tab_window(tab) };
    let cx = CX.with_borrow(Clone::clone)?;
    // Read without an update, as every key a page gets comes here.
    let holds = cx.try_read_global(|preview: &link_preview::LinkPreviewWindow, cx| {
        preview.holds(window, tab, cx)
    });
    if holds != Some(true) {
        return None;
    }
    let keystroke = keystroke(unsafe { &*key });
    cx.update(|cx| link_preview::key(window, tab, &keystroke, reserved, cx))
}

/// The toast's requests, to the host's toast window.
struct ToastHost {
    /// The toast's view, for the share menu its Share button shows, once its window is open.
    view: Cell<*mut c_void>,
}

impl toast::Host for ToastHost {
    fn show(&self) {
        unsafe { platform::host_show_toast(front().1) };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_toast() };
    }

    fn take_clicks(&self, takes: bool) {
        unsafe { platform::host_toast_takes_clicks(takes) };
    }

    fn plate(&self, bounds: Bounds<Pixels>) {
        let (x, y) = (bounds.origin.x.into(), bounds.origin.y.into());
        let (width, height) = (bounds.size.width.into(), bounds.size.height.into());
        // check.sh reads this line, to click just outside the plate.
        log!("browser: toast plate {x:.0} {y:.0} {width:.0} {height:.0}");
        unsafe { platform::host_set_toast_plate(x, y, width, height) };
    }
}

/// Capture's requests, as calls to the core, to the host's window for capture mode and to the
/// system, over the window in front (`front`).
struct CaptureHost {
    /// The toast's view, which its Share button's menu shows from.
    toast: Rc<ToastHost>,
}

impl capture::Host for CaptureHost {
    fn show(&self, frame: Bounds<Pixels>) {
        let (left, top) = (f64::from(frame.origin.x), f64::from(frame.origin.y));
        let (width, height) = (f64::from(frame.size.width), f64::from(frame.size.height));
        unsafe { platform::host_show_capture(front().1, left, top, width, height) };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_capture() };
    }

    fn camera(&self, camera: bool) {
        unsafe { platform::host_capture_camera(camera) };
    }

    fn element_at(&self, tab: u64, at: Point<Pixels>) {
        unsafe { bc::bc_tab_element_at(tab, f64::from(at.x), f64::from(at.y)) };
    }

    fn capture(&self, tab: u64, request: u64, full_page: bool, rect: Option<Bounds<Pixels>>) {
        let rect = rect.map_or(bc::bc_rect { x: 0., y: 0., width: 0., height: 0. }, |rect| {
            bc::bc_rect {
                x: f64::from(rect.origin.x),
                y: f64::from(rect.origin.y),
                width: f64::from(rect.size.width),
                height: f64::from(rect.size.height),
            }
        });
        unsafe { bc::bc_tab_capture(tab, request, full_page, rect) };
    }

    fn end(&self, tab: u64) {
        unsafe { bc::bc_tab_capture_end(tab) };
    }

    fn copy(&self, png: &[u8]) {
        #[cfg(target_os = "linux")]
        system::copy_png(png);
        #[cfg(not(target_os = "linux"))]
        unsafe { platform::host_copy_png(png.as_ptr(), png.len()) };
    }

    fn save(&self, png: &[u8]) {
        #[cfg(target_os = "linux")]
        system::save_png(png);
        #[cfg(not(target_os = "linux"))]
        unsafe { platform::host_save_png(front().1, png.as_ptr(), png.len()) };
    }

    fn save_to_library(&self, png: &[u8]) {
        let (name, png) = (capture_name(), png.to_vec());
        // After the update under way, which the capture's menu is in.
        update_app_later(move |cx| {
            // A picture of a whole screen is megabytes, too many to write on the main thread.
            let saved = cx.background_spawn(async move { save_capture(&name, &png) });
            cx.spawn(async move |cx| {
                if saved.await {
                    cx.update(list_captures);
                }
            })
            .detach();
        });
    }

    fn share(&self, png: &[u8], share: capture::Share) {
        let (view, at, messages) = match share {
            capture::Share::Menu(at) => (front().1, at, false),
            capture::Share::Toast(at) => {
                (self.toast.view.get(), Bounds::new(at, size(px(1.), px(1.))), false)
            }
            capture::Share::Messages => (std::ptr::null_mut(), Bounds::default(), true),
        };
        let (x, y) = (f64::from(at.origin.x), f64::from(at.origin.y));
        let (width, height) = (f64::from(at.size.width), f64::from(at.size.height));
        unsafe {
            platform::host_share_png(png.as_ptr(), png.len(), view, x, y, width, height, messages)
        };
    }
}

/// Where the Library's captures are.
fn media_dir() -> PathBuf {
    PathBuf::from(profile_dir()).join("Media")
}

/// Saves a capture into the Library as `name`, and says whether it did.
fn save_capture(name: &str, png: &[u8]) -> bool {
    let media = media_dir();
    // Two captures in a second each keep theirs, as the system's screenshots do.
    let stem = name.trim_end_matches(".png");
    let path = (1..)
        .map(|n| match n {
            1 => media.join(name),
            n => media.join(format!("{stem} ({n}).png")),
        })
        .find(|path| !path.exists())
        .unwrap_or_default();
    match std::fs::create_dir_all(&media).and_then(|_| std::fs::write(&path, png)) {
        Ok(()) => {
            // check.sh reads this line.
            log!("browser: saved to library {}", path.display());
            true
        }
        Err(error) => {
            log!("couldn't save to the library at {}: {error}", path.display());
            false
        }
    }
}

/// Lists the Library's captures, newest first, away from the main thread, for every window.
fn list_captures(cx: &mut App) {
    let listed = cx.background_spawn(async {
        let entries = std::fs::read_dir(media_dir()).into_iter().flatten().flatten();
        let mut captures: Vec<_> = entries
            .filter_map(|entry| {
                let modified = entry.metadata().ok()?.modified().ok()?;
                let saved = modified.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis();
                let name = entry.file_name().into_string().ok()?;
                let path = entry.path().into_os_string().into_string().ok()?;
                Some(shell::Captured { path: path.into(), name: name.into(), saved: saved as f64 })
            })
            .collect();
        captures.sort_by(|a, b| b.saved.total_cmp(&a.saved));
        captures
    });
    cx.spawn(async move |_| {
        let captures = listed.await;
        // check.sh reads this line.
        log!("browser: library captures {}", captures.len());
        update_shells(|shell, cx| shell.set_captures(captures.clone(), cx));
    })
    .detach();
}

/// A capture's file name, as the system names a screenshot.
pub(crate) fn capture_name() -> String {
    let mut name = [0 as c_char; 64];
    unsafe { platform::host_capture_name(name.as_mut_ptr(), name.len()) };
    unsafe { CStr::from_ptr(name.as_ptr()) }.to_string_lossy().into_owned()
}

/// The user left capture mode: a click outside it, another window or another app.
extern "C" fn capture_dismissed() {
    update_app(capture::dismissed);
}

unsafe extern "C" fn tab_element_box(
    _: *mut c_void,
    tab: bc::bc_id,
    x: f64,
    y: f64,
    box_: bc::bc_rect,
) {
    let at = point(px(x as f32), px(y as f32));
    let found = (box_.width > 0. && box_.height > 0.).then(|| {
        Bounds::new(
            point(px(box_.x as f32), px(box_.y as f32)),
            size(px(box_.width as f32), px(box_.height as f32)),
        )
    });
    update_app_later(move |cx| capture::element_box(tab, at, found, cx));
}

unsafe extern "C" fn tab_captured(
    _: *mut c_void,
    tab: bc::bc_id,
    request: u64,
    png: *const u8,
    size: usize,
    width: i32,
    height: i32,
) {
    // check.sh reads this line.
    log!("browser: tab {tab} captured {request}: {width}x{height}, {size} bytes");
    let png = (!png.is_null()).then(|| unsafe { std::slice::from_raw_parts(png, size) }.to_vec());
    update_app_later(move |cx| capture::captured(tab, request, png, cx));
}

unsafe extern "C" fn trace_recorded(_: *mut c_void, json: *const c_char, size: usize) {
    // check.sh reads this line.
    log!("browser: trace recorded {size} bytes");
    let json = (!json.is_null()).then(|| unsafe { std::slice::from_raw_parts(json.cast(), size) });
    let json = json.map(<[u8]>::to_vec);
    update_app_later(move |cx| shell::trace_recorded(json, cx));
}

unsafe extern "C" fn tab_pictured(
    _: *mut c_void,
    tab: bc::bc_id,
    request: u64,
    bgra: *const u8,
    width: i32,
    height: i32,
) {
    // check.sh reads this line.
    log!("browser: tab {tab} pictured {request}: {width}x{height}");
    if hover_card::is_card_request(request) {
        let (width, height) = (width.max(0) as u32, height.max(0) as u32);
        let size = width as usize * height as usize * 4;
        let picture = (!bgra.is_null() && size > 0)
            .then(|| unsafe { std::slice::from_raw_parts(bgra, size) }.to_vec())
            .and_then(|bgra| PagePicture::from_bgra(bgra, width, height));
        return update_app_later(move |cx| hover_card::answered(request, picture, cx));
    }
    browser::log_handover("picture in", &format!("tab {tab}, {width}x{height}"));
    let (width, height) = (width.max(0) as u32, height.max(0) as u32);
    let size = width as usize * height as usize * 4;
    let picture = (!bgra.is_null() && size > 0)
        .then(|| unsafe { std::slice::from_raw_parts(bgra, size) }.to_vec())
        .and_then(|bgra| PagePicture::from_bgra(bgra, width, height));
    if let Some(cx) = CX.with_borrow(Clone::clone) {
        let shells =
            WINDOWS.with_borrow(|windows| windows.iter().map(|window| window.shell.clone()).collect());
        shell::picture_answered(shells, request, picture, &cx);
    }
}

/// The zoom HUD's requests, to the host's HUD window, over the browser window it names.
struct HudHost;

impl zoom_hud::Host for HudHost {
    fn show(&self, shell: &WeakEntity<Shell>) {
        let ui_view = WINDOWS.with_borrow(|windows| {
            let mut windows = windows.iter();
            let window = windows.find(|window| window.shell.entity_id() == shell.entity_id());
            window.map(|window| window.ui_view)
        });
        if let Some(ui_view) = ui_view {
            unsafe { platform::host_show_hud(ui_view) };
        }
    }

    fn hide(&self) {
        unsafe { platform::host_hide_hud() };
    }
}

/// A browser window's password offer's requests, to the host's offer window over it and the core.
struct OfferHost {
    ui_view: *mut c_void,
}

impl offer::Host for OfferHost {
    fn show(&self) {
        unsafe { platform::host_show_offer(self.ui_view) };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_offer(self.ui_view) };
    }

    fn answer(&self, tab: u64, answer: offer::Answer, username: &str) {
        let answer = match answer {
            offer::Answer::Accept => bc::BC_OFFER_ACCEPT,
            offer::Answer::Never => bc::BC_OFFER_NEVER,
            offer::Answer::NotNow => bc::BC_OFFER_NOT_NOW,
        };
        let username = CString::new(username).unwrap_or_default();
        unsafe { bc::bc_tab_answer_offer(tab, answer as i32, username.as_ptr()) };
    }
}

/// The command bar's requests, as calls to the core and to the host's overlay window, for the
/// window it shows over (`front`).
struct Bar;

impl command_bar::Host for Bar {
    fn suggest(&self, site: Option<&str>, text: &str, complete: bool) {
        let Ok(site) = site.map(CString::new).transpose() else {
            return;
        };
        let site = site.as_ref().map_or(std::ptr::null(), |site| site.as_ptr());
        if let Ok(text) = CString::new(text) {
            unsafe { bc::bc_window_suggest(front().0, site, text.as_ptr(), complete) };
        }
    }

    fn remove_suggestion(&self, url: &str) {
        if let Ok(url) = CString::new(url) {
            unsafe { bc::bc_window_remove_suggestion(front().0, url.as_ptr()) };
        }
    }

    fn typed_url(&self, text: &str) -> Option<String> {
        let window = front().0;
        url_for(text, |text, url, capacity| unsafe { bc::bc_typed_url(window, text, url, capacity) })
    }

    fn search_url(&self, site: Option<&str>, text: &str) -> Option<String> {
        let site = site.map(CString::new).transpose().ok()?;
        let site = site.as_ref().map_or(std::ptr::null(), |site| site.as_ptr());
        let window = front().0;
        url_for(text, |text, url, capacity| unsafe {
            bc::bc_search_url(window, site, text, url, capacity)
        })
    }

    fn page_favicon(&self, url: &str) {
        if let Ok(url) = CString::new(url) {
            unsafe { bc::bc_page_favicon(url.as_ptr()) };
        }
    }

    fn open_tab(&self, url: &str, behind: bool) {
        match behind {
            // ⌘↩'s tab loads once it's first shown, as a link opened behind does (Dia).
            true => _ = open_tab_behind(front().0, url, true),
            false => open_tab(front().0, url),
        }
    }

    fn open_window(&self, url: &str, cx: &mut App) {
        let (id, url) = (front().0, url.to_string());
        // Outside the bar's update, as ⌘N's window opens outside the shell's.
        cx.defer(move |cx| {
            if let Some(window) = open_window_after(id, cx) {
                open_tab(window, &url);
            }
        });
    }

    fn navigate(&self, tab: u64, url: &str) {
        if let Ok(url) = CString::new(url) {
            unsafe { bc::bc_tab_navigate(tab, url.as_ptr()) };
        }
    }

    fn reload(&self, tab: u64) {
        unsafe { bc::bc_tab_reload(tab, false) };
    }

    fn select(&self, tab: u64) {
        unsafe { bc::bc_window_select(front().0, tab) };
    }

    fn tab_window(&self, tab: u64) -> Option<u64> {
        let window = unsafe { bc::bc_tab_window(tab) };
        (window != 0 && window != front().0).then_some(window)
    }

    fn activate(&self, window: u64, tab: u64, cx: &mut App) {
        let Some(ui_view) = WINDOWS.with_borrow(|windows| {
            windows.iter().find(|open| open.id == window).map(|open| open.ui_view)
        }) else {
            return;
        };
        // Outside any update: the window reports that it's key at once, to its shell.
        cx.spawn(async move |_| unsafe {
            bc::bc_window_select(window, tab);
            platform::host_activate(ui_view);
        })
        .detach();
    }

    fn split(&self, tab: u64, with: u64, cx: &mut App) {
        split_tab(front().0, tab, with, false, cx);
    }

    fn open_split(&self, url: &str, with: u64, cx: &mut App) {
        open_split(front().0, url, with, cx);
    }

    fn copy(&self, text: &str, cx: &mut App) {
        shell::copy_from_bar(text, cx);
    }

    fn show(&self, centered: bool, left: f32, top: f32, width: f32, radius: f32, level: Level) {
        let (left, top, width) = (f64::from(left), f64::from(top), f64::from(width));
        elevate(COMMAND_BAR, level);
        unsafe {
            platform::host_set_overlay_radius(COMMAND_BAR, radius.into());
            platform::host_show_overlay(COMMAND_BAR, front().1, centered, left, top, width)
        };
    }

    fn set_height(&self, height: f32) {
        unsafe { platform::host_set_overlay_height(COMMAND_BAR, f64::from(height)) };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_overlay(COMMAND_BAR) };
    }

    fn dismiss(&self) {
        unsafe { platform::host_dismiss_overlay(COMMAND_BAR) };
    }

    fn take_keyboard(&self) {
        // Linux's and Windows' hosts close the bar once its keyboard goes elsewhere.
        #[cfg(target_os = "macos")]
        unsafe {
            platform::host_take_overlay_keyboard(COMMAND_BAR)
        };
    }

    fn commands(&self, text: &str, cx: &App) -> Vec<shell::BarCommand> {
        shell::bar_commands(text, cx)
    }
}

/// The user left the command bar without choosing anything in it.
extern "C" fn command_bar_dismissed() {
    update_app(command_bar::dismissed);
    update_window(front().0, |shell, cx| shell.command_bar_dismissed(cx));
}

/// The icon picker's requests, to the host's overlay window, over the window in front.
struct PickerHost;

impl icon_picker::Host for PickerHost {
    fn show(&self, left: f32, top: f32) {
        let (left, top, width) = (left.into(), top.into(), icon_picker::WIDTH.into());
        unsafe { platform::host_show_overlay(ICON_PICKER, front().1, false, left, top, width) };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_overlay(ICON_PICKER) };
    }

    fn keep_recents(&self, recents: &str) {
        if let Err(error) = std::fs::write(recent_emoji_path(), recents) {
            log!("couldn't save the recent emoji: {error}");
        }
    }

    fn draws(&self, emoji: &str) -> bool {
        // An emoji has no NUL, so the string always converts.
        std::ffi::CString::new(emoji)
            .is_ok_and(|emoji| unsafe { platform::host_can_draw_emoji(emoji.as_ptr()) })
    }
}

/// Where the emoji the icon picker offers first are kept: in the profile directory beside the
/// shortcuts, as the core keeps no settings of the app's own.
fn recent_emoji_path() -> String {
    format!("{}/Recent Emoji", profile_dir())
}

fn recent_emoji() -> icon_picker::Recents {
    icon_picker::Recents::parse(&std::fs::read_to_string(recent_emoji_path()).unwrap_or_default())
}

/// The user left the icon picker without picking.
extern "C" fn icon_picker_dismissed() {
    update_app(icon_picker::dismissed);
}

struct FolderPreviewHost;

impl folder_preview::Host for FolderPreviewHost {
    fn show(&self, left: f32, top: f32, height: f32) {
        let (left, top, width) = (left.into(), top.into(), folder_preview::WIDTH.into());
        unsafe {
            platform::host_show_overlay(FOLDER_PREVIEW, front().1, false, left, top, width);
            platform::host_set_overlay_height(FOLDER_PREVIEW, height.into());
        }
    }

    fn hide(&self) {
        unsafe { platform::host_hide_overlay(FOLDER_PREVIEW) };
    }
}

extern "C" fn folder_preview_dismissed() {
    update_app(folder_preview::dismissed);
}

/// The theme picker's requests, to the host's overlay window, over the window in front.
struct ThemeHost;

impl theme_picker::Host for ThemeHost {
    fn show(&self, left: f32, top: f32, corners: &[(f32, f32, f32)]) {
        let shape: Vec<f64> =
            corners.iter().flat_map(|&(x, y, radius)| [x, y, radius].map(f64::from)).collect();
        // The arrow, which it grows in from, moves along its side to point at what opened it.
        let (x, y, _) = corners[0];
        let (left, top, width) = (left.into(), top.into(), theme_picker::WIDTH.into());
        unsafe {
            platform::host_set_overlay_shape(THEME_PICKER, shape.as_ptr(), shape.len() / 3);
            platform::host_set_overlay_growth(THEME_PICKER, x.into(), y.into());
            platform::host_show_overlay(THEME_PICKER, front().1, false, left, top, width);
        }
    }

    fn hide(&self) {
        unsafe { platform::host_hide_overlay(THEME_PICKER) };
    }

    fn detent(&self) {
        if unsafe { bc::bc_haptics() } {
            unsafe { platform::host_detent() };
        }
    }
}

/// The user left the theme picker, such as with a click outside it.
extern "C" fn theme_picker_dismissed() {
    update_app(theme_picker::dismissed);
}

/// The tooltip's requests, to the host's tooltip window.
struct TooltipHost;

impl tooltip::Host for TooltipHost {
    fn show(&self, left: f32, top: f32) {
        let fade = tooltip::FADE_IN.as_secs_f64();
        unsafe { platform::host_show_tooltip(front().1, left.into(), top.into(), fade) };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_tooltip(tooltip::FADE_OUT.as_secs_f64()) };
    }
}

/// The window lost the keyboard, or the app stopped being the active one.
extern "C" fn tooltip_away() {
    // Later: the window resigns key from inside the update that opened another, such as Settings.
    update_app_later(tooltip::dismiss);
}

/// The hover card's requests, to the host's card window, over the browser window in front.
struct CardHost;

impl hover_card::Host for CardHost {
    fn show(&self, left: f32, top: f32, width: f32, height: f32) {
        let (left, top, width, height) = (left.into(), top.into(), width.into(), height.into());
        unsafe { platform::host_show_hover_card(front().1, left, top, width, height) };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_hover_card() };
    }
}

/// The window lost the keyboard, the app stopped being the active one, or a press or a scroll
/// landed outside the card.
extern "C" fn card_away() {
    // Later, as tooltip_away.
    update_app_later(hover_card::close);
}

/// The system is short of memory: the hover cards' pictures, which the core can take again, go.
extern "C" fn memory_pressed() {
    update_app(hover_card::forget_all);
}

/// The link target's requests, to the host's link target window.
struct LinkTargetHost;

impl link_target::Host for LinkTargetHost {
    fn show(&self, left: f32, top: f32) {
        let fade = link_target::FADE_IN.as_secs_f64();
        unsafe { platform::host_show_link_target(front().1, left.into(), top.into(), fade) };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_link_target(link_target::FADE_OUT.as_secs_f64()) };
    }
}

/// The pointer moved, at (`x`, `y`) in the window in front, while the link target shows.
extern "C" fn link_pointer(x: f64, y: f64) {
    update_window(front().0, |shell, cx| shell.link_pointer(x as f32, y as f32, cx));
}

/// The pointer is on a link in `tab`'s page, or has left it and `url` is "".
unsafe extern "C" fn tab_link_target(
    _: *mut c_void,
    tab: bc::bc_id,
    url: *const c_char,
    domain_begin: usize,
    domain_end: usize,
) {
    let url = unsafe { CStr::from_ptr(url) }.to_string_lossy().into_owned();
    // check.sh reads this line.
    log!("browser: link target {}", if url.is_empty() { "none" } else { url.as_str() });
    let target = (!url.is_empty())
        .then(|| link_target::Target { text: url.into(), domain: domain_begin..domain_end });
    // After the update under way, if any: the core also says it when a page navigates, inside the
    // call that navigated.
    update_app_later(move |cx| {
        if let Some(window) = tab_window(tab, cx) {
            window.shell.update(cx, |shell, cx| shell.link_target_changed(tab, target, cx)).ok();
        }
    });
}

/// Gives the keyboard in `window` to the UI, whose GPUI view is `view`, to keep or, with
/// `until_page`, until a page shows. On Linux the core says whose the keys are, and takes them from
/// the page. On Windows it hears first, while the focus still says whether the UI had them.
fn focus_ui(window: bc::bc_id, view: *mut c_void, until_page: bool) {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    let tell_core = || unsafe {
        if until_page {
            bc::bc_window_ui_holds_keyboard(window)
        } else {
            bc::bc_window_ui_takes_keyboard(window)
        }
    };
    #[cfg(target_os = "windows")]
    tell_core();
    unsafe { platform::host_focus(view) };
    #[cfg(target_os = "linux")]
    tell_core();
    #[cfg(target_os = "macos")]
    let _ = (window, until_page);
}

/// A window's shell's requests, as calls to the core. The core reports back once each call has
/// returned.
struct Engine {
    /// The window's, in the core.
    window: bc::bc_id,
    /// The view the UI draws in, for the host.
    ui_view: *mut c_void,
    /// GPUI's view, which has the keyboard when the UI does.
    view: *mut c_void,
    /// The videos Media's tiles play, by tile.
    videos: RefCell<HashMap<u64, Video>>,
}

/// A video a Media tile plays, from the host, with the pixels its frames are copied into.
struct Video {
    player: *mut c_void,
    side: u32,
    pixels: Vec<u8>,
}

impl Drop for Engine {
    fn drop(&mut self) {
        for (_, video) in self.videos.take() {
            unsafe { platform::host_video_close(video.player) };
        }
    }
}

impl Browser for Engine {
    fn select(&self, tab: u64, _: &mut App) {
        browser::handover_clicked();
        unsafe { bc::bc_window_select(self.window, tab) };
    }

    fn close(&self, tab: u64, split: bool, _: &mut App) {
        unsafe { bc::bc_tab_close(tab, split) };
    }

    fn close_below(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_close_below(tab) };
    }

    fn duplicate(&self, tab: u64, _: &mut App) -> u64 {
        unsafe { bc::bc_tab_duplicate(tab) }
    }

    fn move_to_space(&self, tab: u64, space: u64, _: &mut App) {
        unsafe { bc::bc_tab_move_to_space(tab, space) };
    }

    fn warn_moving_profiles(&self, _: &mut App) -> bool {
        unsafe { bc::bc_warn_moving_profiles() }
    }

    fn set_warn_moving_profiles(&self, warn: bool, _: &mut App) {
        unsafe { bc::bc_set_warn_moving_profiles(warn) };
    }

    fn set_pinned(&self, tab: u64, pinned: bool, _: &mut App) {
        unsafe { bc::bc_tab_set_pinned(tab, pinned) };
    }

    fn move_to_favorites(&self, tab: u64, index: usize, _: &mut App) {
        unsafe { bc::bc_tab_move_to_favorites(tab, index) };
    }

    fn move_tab(&self, tab: u64, pinned: bool, folder: u64, index: usize, _: &mut App) {
        unsafe { bc::bc_tab_move(tab, pinned, folder, index) };
    }

    // The ones after the first load once shown, as links opened behind do.
    fn open_at(
        &self,
        urls: &[String],
        pinned: bool,
        folder: u64,
        index: usize,
        _: &mut App,
    ) -> u64 {
        let tabs = urls
            .iter()
            .enumerate()
            .map(|(at, url)| (at, open_tab_behind(self.window, url, at > 0)));
        let tabs: Vec<u64> = tabs.filter(|&(_, tab)| tab != 0).map(|(_, tab)| tab).collect();
        for (at, &tab) in tabs.iter().enumerate() {
            unsafe { bc::bc_tab_move(tab, pinned, folder, index + at) };
        }
        let first = tabs.first().copied().unwrap_or(0);
        if first != 0 {
            unsafe { bc::bc_window_select(self.window, first) };
        }
        first
    }

    fn open_behind(&self, urls: &[String], pinned: bool, folder: u64, index: usize, _: &mut App) {
        let tabs = urls.iter().map(|url| open_tab_behind(self.window, url, true));
        for (at, tab) in tabs.filter(|&tab| tab != 0).enumerate() {
            unsafe { bc::bc_tab_move(tab, pinned, folder, index + at) };
        }
    }

    fn create_folder(&self, space: u64, name: &str, folder: u64, index: usize, _: &mut App) -> u64 {
        let name = CString::new(name).unwrap_or_default();
        unsafe { bc::bc_folder_create(space, name.as_ptr(), folder, index) }
    }

    fn rename_folder(&self, folder: u64, name: &str, _: &mut App) {
        let name = CString::new(name).unwrap_or_default();
        unsafe { bc::bc_folder_rename(folder, name.as_ptr()) };
    }

    fn set_folder_icon(&self, folder: u64, icon: &str, _: &mut App) {
        let icon = CString::new(icon).unwrap_or_default();
        unsafe { bc::bc_folder_set_icon(folder, icon.as_ptr()) };
    }

    fn duplicate_folder(&self, folder: u64, _: &mut App) -> u64 {
        unsafe { bc::bc_folder_duplicate(folder) }
    }

    fn set_folder_open(&self, folder: u64, open: bool, _: &mut App) {
        unsafe { bc::bc_folder_set_open(folder, open) };
    }

    fn set_pinned_open(&self, space: u64, open: bool, _: &mut App) {
        unsafe { bc::bc_space_set_pinned_open(space, open) };
    }

    fn move_folder(&self, folder: u64, to: u64, index: usize, _: &mut App) {
        unsafe { bc::bc_folder_move(folder, to, index) };
    }

    fn delete_folder(&self, folder: u64, _: &mut App) {
        unsafe { bc::bc_folder_delete(folder) };
    }

    fn separate(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_separate(tab) };
    }

    fn move_in_split(&self, tab: u64, index: usize, _: &mut App) {
        unsafe { bc::bc_tab_move_in_split(tab, index) };
    }

    fn set_split_stacked(&self, tab: u64, stacked: bool, _: &mut App) {
        unsafe { bc::bc_tab_set_split_stacked(tab, stacked) };
    }

    fn set_split_shares(&self, tab: u64, shares: Option<&[f64]>, _: &mut App) {
        let (shares, count) = shares.map_or((std::ptr::null(), 0), |shares| {
            (shares.as_ptr(), shares.len())
        });
        unsafe { bc::bc_tab_set_split_shares(tab, shares, count) };
    }

    fn set_split_expanded(&self, tab: u64, expanded: bool, _: &mut App) {
        unsafe { bc::bc_tab_set_split_expanded(tab, expanded) };
    }

    fn split_with(&self, tab: u64, with: u64, left: bool, _: &mut App) {
        unsafe {
            bc::bc_tab_split_with(tab, with, left);
            bc::bc_window_select(self.window, tab);
        }
    }

    fn open_split_at(&self, url: &str, with: u64, left: bool, stacked: bool, _: &mut App) {
        let (window, url) = (self.window, url.to_string());
        // After the drop that asked, which Chromium is still handling.
        update_app_later(move |cx| {
            let tab = open_tab_behind(window, &url, false);
            if tab == 0 {
                return;
            }
            let joined = unsafe { bc::bc_tab_split_with(tab, with, left) };
            unsafe {
                if stacked {
                    bc::bc_tab_set_split_stacked(with, true);
                }
                bc::bc_window_select(window, tab);
            }
            if joined {
                split_opened(window, tab, cx);
            }
        });
    }

    fn show_drop_targets(&self, shown: bool, cx: &mut App) {
        let ui_view = self.ui_view;
        // After the shell's draw, which asks for it.
        cx.defer(move |cx| drop_targets::show(shown, ui_view, cx));
    }

    fn back_to_pinned_url(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_back_to_pinned_url(tab) };
    }

    fn set_muted(&self, tab: u64, muted: bool, _: &mut App) {
        unsafe { bc::bc_tab_set_muted(tab, muted) };
    }

    fn share_instead(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_share_instead(tab) };
    }

    fn media(&self, tab: u64, action: MediaAction, _: &mut App) {
        let action = match action {
            MediaAction::Play => bc::BC_MEDIA_PLAY,
            MediaAction::Pause => bc::BC_MEDIA_PAUSE,
            MediaAction::Next => bc::BC_MEDIA_NEXT,
            MediaAction::PictureInPicture => bc::BC_MEDIA_PICTURE_IN_PICTURE,
        };
        unsafe { bc::bc_tab_media(tab, action as i32) };
    }

    fn cast(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_cast(tab) };
    }

    fn cast_close(&self, _: &mut App) {
        unsafe { bc::bc_cast_close() };
    }

    fn cast_to(&self, tab: u64, id: SharedString, _: &mut App) {
        let id = CString::new(id.as_ref()).unwrap_or_default();
        unsafe { bc::bc_tab_cast_to(tab, id.as_ptr()) };
    }

    fn clear(&self, _: &mut App) -> u64 {
        unsafe { bc::bc_window_clear(self.window) }
    }

    fn undo_clear(&self, clear: u64, _: &mut App) -> usize {
        unsafe { bc::bc_window_undo_clear(self.window, clear) }
    }

    fn undo_delete_space(&self, space: u64, _: &mut App) -> bool {
        unsafe { bc::bc_window_undo_delete_space(self.window, space) != 0 }
    }

    fn reopen_closed_tab(&self, _: &mut App) {
        unsafe { bc::bc_window_reopen_closed_tab(self.window) };
    }

    fn reopen_tab(&self, closed: u64, _: &mut App) -> u64 {
        unsafe { bc::bc_window_reopen_tab(self.window, closed) }
    }

    fn reopen_preview(
        &self,
        closed: &link_preview::Closed,
        page: (Bounds<Pixels>, f32),
        _: &mut App,
    ) {
        let (window, closed) = (self.window, closed.clone());
        // Outside the shell's update, as the core tells the shell of the tab it opens.
        update_app_later(move |cx| {
            let Ok(url) = CString::new(closed.url.as_ref()) else {
                return;
            };
            let preview = unsafe { bc::bc_tab_open_preview(closed.over, url.as_ptr()) };
            if preview == 0 {
                log!("browser: link preview over {} not reopened", closed.over);
                return;
            }
            link_preview::show(window, preview, closed.over, closed.url, page, closed.link, cx);
        });
    }

    fn close_preview(&self, over: u64, cx: &mut App) -> bool {
        link_preview::close_over(self.window, over, cx)
    }

    fn pinned_url(&self, tab: u64) -> String {
        name(bc::bc_tab_pinned_url, tab).to_string()
    }

    fn typed_url(&self, text: &str) -> Option<String> {
        let window = self.window;
        url_for(text, |text, url, capacity| unsafe { bc::bc_typed_url(window, text, url, capacity) })
    }

    fn set_pinned_url(&self, tab: u64, url: &str, _: &mut App) {
        let url = CString::new(url).unwrap_or_default();
        unsafe { bc::bc_tab_set_pinned_url(tab, url.as_ptr()) };
    }

    fn tab_name(&self, tab: u64) -> String {
        name(bc::bc_tab_name, tab).to_string()
    }

    fn undo_archive_remove(&self, removal: u64, _: &mut App) -> usize {
        unsafe { bc::bc_archive_undo_remove(removal) }
    }

    fn restore_archived(&self, archived: u64, _: &mut App) {
        unsafe { bc::bc_window_restore_archived(self.window, archived) };
    }

    fn act_on_download(&self, download: u64, action: DownloadAction, _: &mut App) {
        let action = match action {
            DownloadAction::Open => bc::BC_DOWNLOAD_OPEN,
            DownloadAction::Show => bc::BC_DOWNLOAD_SHOW,
            DownloadAction::Pause => bc::BC_DOWNLOAD_PAUSE,
            DownloadAction::Resume => bc::BC_DOWNLOAD_RESUME,
            DownloadAction::Cancel => bc::BC_DOWNLOAD_CANCEL,
            DownloadAction::Remove => bc::BC_DOWNLOAD_REMOVE,
            DownloadAction::Keep => bc::BC_DOWNLOAD_KEEP,
        };
        unsafe { bc::bc_download_act(download, action as i32) };
    }

    fn clear_downloads(&self, _: &mut App) {
        unsafe { bc::bc_downloads_clear() };
    }

    fn download_icon(&self, download: u64, _: &mut App) {
        unsafe { bc::bc_download_icon(download) };
    }

    fn file_preview(&self, download: u64, path: &str, _: &mut App) {
        if let Ok(path) = CString::new(path) {
            unsafe { platform::host_file_preview(path.as_ptr(), download, file_preview) };
        }
    }

    fn open_video(&self, id: u64, path: &str, side: u32, _: &mut App) -> bool {
        let Ok(path) = CString::new(path) else {
            return false;
        };
        let player = unsafe { platform::host_video_open(path.as_ptr(), side as c_int) };
        if player.is_null() {
            return false;
        }
        let pixels = vec![0; side as usize * side as usize * 4];
        self.videos.borrow_mut().insert(id, Video { player, side, pixels });
        true
    }

    fn video_frame(&self, id: u64, _: &mut App) -> Option<Arc<gpui::RenderImage>> {
        let mut videos = self.videos.borrow_mut();
        let video = videos.get_mut(&id)?;
        if !unsafe { platform::host_video_frame(video.player, video.pixels.as_mut_ptr()) } {
            return None;
        }
        // BGRA, as GPUI draws a RenderImage's pixels.
        let frame = image::RgbaImage::from_raw(video.side, video.side, video.pixels.clone())?;
        Some(Arc::new(gpui::RenderImage::new(vec![image::Frame::new(frame)])))
    }

    fn close_video(&self, id: u64, _: &mut App) {
        if let Some(video) = self.videos.borrow_mut().remove(&id) {
            unsafe { platform::host_video_close(video.player) };
        }
    }

    fn list_captures(&self, cx: &mut App) {
        list_captures(cx);
    }

    fn hover_kinds(&self, _: &mut App) -> u32 {
        unsafe { bc::bc_hover_kinds() as u32 }
    }

    fn set_hover_kinds(&self, kinds: u32, _: &mut App) {
        unsafe { bc::bc_set_hover_kinds(kinds as c_int) };
    }

    fn trash(&self, path: &str, _: &mut App) -> bool {
        CString::new(path).is_ok_and(|path| unsafe { platform::host_trash(path.as_ptr()) })
    }

    fn drag_file(&self, path: &str, _: &mut App) -> bool {
        CString::new(path)
            .is_ok_and(|path| unsafe { platform::host_drag_file(path.as_ptr(), file_drag_ended) })
    }

    fn remove_archived(&self, archived: u64, _: &mut App) -> u64 {
        unsafe { bc::bc_archive_remove(archived) }
    }

    fn clear_archive(&self, _: &mut App) -> u64 {
        unsafe { bc::bc_archive_clear() }
    }

    fn page_favicon(&self, url: &str, _: &mut App) {
        if let Ok(url) = CString::new(url) {
            unsafe { bc::bc_page_favicon(url.as_ptr()) };
        }
    }

    fn day_starts(&self) -> [f64; 4] {
        let mut starts = [0.; 4];
        unsafe { platform::host_day_starts(starts.as_mut_ptr()) };
        starts
    }

    fn go_back(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_go_back(tab) };
    }

    fn go_forward(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_go_forward(tab) };
    }

    fn tab_history(&self, tab: u64, forward: bool, _: &mut App) {
        unsafe { bc::bc_tab_history(tab, forward) };
    }

    fn go_to_history(&self, tab: u64, offset: i32, _: &mut App) {
        unsafe { bc::bc_tab_go_to_history(tab, offset) };
    }

    fn open_history(&self, tab: u64, offset: i32, _: &mut App) {
        unsafe { bc::bc_tab_open_history(tab, offset) };
    }

    fn reload(&self, tab: u64, bypass_cache: bool, _: &mut App) {
        unsafe { bc::bc_tab_reload(tab, bypass_cache) };
    }

    fn clear_site(&self, tab: u64, cookies: bool, _: &mut App) {
        unsafe { bc::bc_tab_clear_site(tab, cookies) };
    }

    fn set_developer_mode(&self, tab: u64, on: bool, _: &mut App) {
        unsafe { bc::bc_tab_set_developer_mode(tab, on) };
    }

    fn answer_permission(&self, tab: u64, answer: shell::Answer, _: &mut App) {
        let answer = match answer {
            shell::Answer::Allow => bc::BC_PERMISSION_ALLOW,
            shell::Answer::Block => bc::BC_PERMISSION_BLOCK,
            shell::Answer::Dismiss => bc::BC_PERMISSION_DISMISS,
        };
        unsafe { bc::bc_tab_answer_permission(tab, answer as i32) };
    }

    fn answer_agent(&self, question: u64, answer: browser::agents::Answer, _: &mut App) {
        // After the shell's update, as the answer tells every window's shell the next question.
        agents_core::update(move |agents, cx| agents.answer(question, answer, cx));
    }

    fn agent_permissions(&self, space: u64, cx: &App) -> shell::AgentGrants {
        let agent = name(bc::bc_space_agent, space);
        let Some(agents) = browser::agents::global(cx) else {
            return Default::default();
        };
        let agents = agents.read(cx);
        let client = browser::agents::Client::from_id(&agent);
        shell::AgentGrants {
            sites: agents.grants(&agent),
            connected: client.is_some_and(|client| agents.is_connected(client)),
            tabs: !agents.tabs_to_take(&agent, cx).is_empty(),
        }
    }

    fn revoke_agent(&self, space: u64, site: Option<SharedString>, _: &mut App) {
        let agent = name(bc::bc_space_agent, space);
        agents_core::update(move |agents, cx| agents.revoke(&agent, site.as_deref(), cx));
    }

    fn take_over(&self, agent: SharedString, tab: u64, _: &mut App) {
        agents_core::update(move |agents, cx| agents.take_over(&agent, tab, cx));
    }

    fn hand_back(&self, tab: u64, _: &mut App) {
        agents_core::update(move |agents, cx| agents.hand_back(tab, cx));
    }

    fn give_to_agent(&self, tab: u64, space: u64, _: &mut App) {
        agents_core::update(move |agents, cx| agents.give(tab, space, cx));
    }

    fn stop_agent(&self, agent: SharedString, _: &mut App) {
        agents_core::update(move |agents, cx| agents.stop_agent(&agent, cx));
    }

    fn disconnect_agent(&self, space: u64, _: &mut App) {
        let agent = name(bc::bc_space_agent, space);
        if let Some(client) = browser::agents::Client::from_id(&agent) {
            agents_core::update(move |agents, cx| agents.disconnect(client, cx));
        }
    }

    fn take_back_agent_tabs(&self, space: u64, _: &mut App) {
        let agent = name(bc::bc_space_agent, space);
        agents_core::update(move |agents, cx| agents.take_back_all(&agent, cx));
    }

    fn show_page_dialog(&self, shown: Option<shell::ShownPageDialog>, cx: &mut App) {
        // The dialog shows over the window last key, as the others do; another's waits for it.
        if self.window != front().0 {
            return;
        }
        let Some((tab, asked, over)) = shown else {
            dialog::withdraw_page(cx);
            return;
        };
        let field = |placeholder: &SharedString, text: &SharedString, secure| dialog::Field {
            placeholder: placeholder.clone(),
            text: text.clone(),
            secure,
        };
        let none = SharedString::default();
        let fields = match asked.kind {
            shell::DialogKind::Prompt => vec![field(&none, &asked.text, false)],
            shell::DialogKind::SignIn => {
                vec![field(&asked.username, &none, false), field(&asked.password, &none, true)]
            }
            _ => Vec::new(),
        };
        let question = dialog::Question {
            icon: None,
            message: asked.title.clone(),
            detail: asked.message.clone(),
            list: Vec::new(),
            action: asked.accept.clone(),
            cancel: asked.cancel.clone(),
            tone: match asked.kind {
                // Leaving loses what the user changed in the page.
                shell::DialogKind::Leave => dialog::Tone::Destructive,
                // Another answer to a page takes nothing back.
                _ => dialog::Tone::Accent,
            },
            also: None,
            fields,
            check: (!asked.check.is_empty()).then(|| asked.check.clone()),
            on_return: match asked.kind == shell::DialogKind::OpenApp {
                true => dialog::Return::Cancel,
                false => dialog::Return::Action,
            },
        };
        let window = self.window;
        let kind = asked.kind;
        let answered = Box::new(move |reply: dialog::Reply, cx: &mut App| {
            let mut texts = reply.texts.into_iter();
            let (text, password) = match kind {
                shell::DialogKind::Prompt => (texts.next().unwrap_or_default(), String::new()),
                shell::DialogKind::SignIn => {
                    (texts.next().unwrap_or_default(), texts.next().unwrap_or_default())
                }
                _ => (String::new(), String::new()),
            };
            let reply =
                shell::PageReply { accept: reply.accept, text, password, checked: reply.checked };
            let shell = WINDOWS.with_borrow(|windows| {
                windows.iter().find(|w| w.id == window).map(|w| w.shell.clone())
            });
            match shell.and_then(|shell| shell.upgrade()) {
                Some(shell) => {
                    shell.update(cx, |shell, cx| shell.answer_page_dialog(tab, asked.id, reply, cx))
                }
                // Its window closed while the dialog left, so the page isn't left waiting.
                None => answer_page_dialog(tab, asked.id, reply),
            }
        });
        dialog::ask_page((tab, asked.id), question, over, answered, cx);
    }

    fn answer_page_dialog(&self, tab: u64, id: u64, reply: shell::PageReply, _: &mut App) {
        answer_page_dialog(tab, id, reply);
    }

    fn show_offer(&self, tab: u64) {
        unsafe { bc::bc_tab_show_offer(tab) };
    }

    fn stop(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_stop(tab) };
    }

    fn zoom(&self, tab: u64, step: i32, cx: &mut App) {
        if tab == 0 {
            return;
        }
        let percent = unsafe { bc::bc_tab_zoom(tab, step) };
        // check.sh reads this line.
        log!("browser: zoom {tab} {percent}%");
        let shell = shell_with_view(self.ui_view);
        with_overlays(move |cx| zoom_hud::show(tab, percent, shell, cx), cx);
    }

    fn view_source(&self, tab: u64, _: &mut App) {
        if tab != 0 {
            unsafe { bc::bc_tab_view_source(tab) };
        }
    }

    fn save_page(&self, tab: u64, _: &mut App) {
        if tab != 0 {
            unsafe { bc::bc_tab_save_page(tab) };
        }
    }

    fn save_markdown(&self, name: &str, markdown: String, links: usize, cx: &mut App) {
        let save = move |path: std::path::PathBuf| match std::fs::write(&path, markdown) {
            // check.sh reads this line.
            Ok(()) => log!("browser: exported {links} links to {}", path.display()),
            Err(error) => log!("couldn't export to {}: {error}", path.display()),
        };
        // A check names its file, as it can't drive the system's panel.
        if !cfg!(feature = "prod")
            && let Some(file) = std::env::var_os("BROWSER_EXPORT_FILE")
        {
            return save(file.into());
        }
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
        let chosen = cx.prompt_for_new_path(&home, Some(name));
        cx.foreground_executor()
            .spawn(async move {
                if let Ok(Ok(Some(path))) = chosen.await {
                    save(path);
                }
            })
            .detach();
    }

    fn close_page_menu(&self, tab: u64, command: Option<i32>, _: &mut App) {
        unsafe { bc::bc_tab_close_page_menu(tab, command.unwrap_or(0)) };
    }

    fn print(&self, tab: u64, _: &mut App) {
        if tab != 0 {
            unsafe { bc::bc_tab_print(tab) };
        }
    }

    fn devtools(&self, tab: u64, what: i32, _: &mut App) {
        if tab != 0 {
            unsafe { bc::bc_tab_devtools(tab, what) };
        }
    }

    fn task_manager(&self, _: &mut App) {
        unsafe { bc::bc_window_task_manager(self.window) };
    }

    fn start_trace(&self) -> bool {
        unsafe { bc::bc_trace_start() }
    }

    fn stop_trace(&self) {
        unsafe { bc::bc_trace_stop() };
    }

    fn trace_note(&self, stop: Option<Rc<dyn Fn(&mut App)>>, cx: &mut App) {
        let Some(stop) = stop else {
            return toast::hide_resting(cx);
        };
        let button = toast::Button {
            label: toast::Label::Text(shell::STOP.into()),
            name: "stop",
            run: Rc::new(move |_, cx| stop(cx)),
        };
        with_overlays(move |cx| toast::show_resting(shell::RECORDING, vec![button], cx), cx);
    }

    fn set_menus(&self, bar: &shell::MenuBar, cx: &mut App) {
        MENUS.with_borrow_mut(|menus| menus.bar = bar.clone());
        // The menu bar is the key window's, which lists its own Space's extensions.
        if front().0 == self.window {
            show_menus(cx);
        }
    }

    fn set_edit_titles(&self, undo: Option<&str>, redo: Option<&str>, cx: &mut App) {
        MENUS.with_borrow_mut(|menus| {
            menus.undo = undo.map(Into::into);
            menus.redo = redo.map(Into::into);
        });
        if front().0 == self.window {
            show_menus(cx);
        }
    }

    fn set_window_title(&self, title: &str, _: &mut App) {
        let title = CString::new(title).unwrap_or_default();
        unsafe { platform::host_set_window_title(self.ui_view, title.as_ptr()) };
    }

    fn run_extension(&self, id: &str, anchor: Bounds<Pixels>, _: &mut App) -> bool {
        let Ok(id) = CString::new(id) else {
            return false;
        };
        unsafe { bc::bc_window_run_extension(self.window, id.as_ptr(), rect(anchor)) }
    }

    fn run_extension_command(&self, id: &str, name: &str, _: &mut App) -> bool {
        let (Ok(id), Ok(name)) = (CString::new(id), CString::new(name)) else {
            return false;
        };
        unsafe { bc::bc_window_run_extension_command(self.window, id.as_ptr(), name.as_ptr()) }
    }

    fn set_extension_pinned(&self, profile: u64, id: &str, pinned: bool, _: &mut App) {
        if let Ok(id) = CString::new(id) {
            unsafe { bc::bc_extension_set_pinned(profile, id.as_ptr(), pinned) };
        }
    }

    fn remove_extension(&self, profile: u64, id: &str, _: &mut App) -> bool {
        let Ok(id) = CString::new(id) else {
            return false;
        };
        unsafe { bc::bc_extension_remove(profile, id.as_ptr()) }
    }

    fn turn_on_extension(&self, profile: u64, id: &str, _: &mut App) -> bool {
        let Ok(id) = CString::new(id) else {
            return false;
        };
        unsafe { bc::bc_extension_turn_on(profile, id.as_ptr()) }
    }

    fn extension_crash_told(&self, profile: u64, id: &str, _: &mut App) {
        if let Ok(id) = CString::new(id) {
            unsafe { bc::bc_extension_crash_told(profile, id.as_ptr()) };
        }
    }

    fn open_extension_options(&self, url: &str, _: &mut App) {
        open_tab(self.window, url);
    }

    fn manage_extensions(&self, id: Option<&str>, _: &mut App) {
        match id {
            Some(id) => open_tab(self.window, &format!("chrome://extensions/?id={id}")),
            None => open_tab(self.window, "chrome://extensions"),
        }
    }

    fn add_extension(&self, _: &mut App) {
        open_tab(self.window, WEB_STORE);
    }

    fn lay_out_side_panel(&self, frame: Bounds<Pixels>, _: &mut App) {
        unsafe { bc::bc_window_set_side_panel_frame(self.window, rect(frame)) };
    }

    fn close_side_panel(&self, _: &mut App) {
        unsafe { bc::bc_window_close_side_panel(self.window) };
    }

    fn has_side_panel(&self, id: &str) -> bool {
        let Ok(id) = CString::new(id) else {
            return false;
        };
        unsafe { bc::bc_window_has_side_panel(self.window, id.as_ptr()) }
    }

    fn toggle_side_panel(&self, id: &str, _: &mut App) {
        if let Ok(id) = CString::new(id) {
            unsafe { bc::bc_window_toggle_side_panel(self.window, id.as_ptr()) };
        }
    }

    fn set_side_panel_width(&self, width: f32, _: &mut App) {
        unsafe { bc::bc_window_set_side_panel_width(self.window, width.into()) };
    }

    fn view_history(&self, _: &mut App) {
        open_tab(self.window, "chrome://history");
    }

    fn open_page(&self, url: &str, _: &mut App) {
        open_tab(self.window, url);
    }

    fn new_note(&self, space: u64, beside: Option<u64>, cx: &mut App) {
        let profile = unsafe { bc::bc_space_profile(space) };
        let url = browser::notes::new_document(&name(bc::bc_profile_notes_service, profile));
        // check.sh waits for this line.
        log!("browser: new note {url}{}", if beside.is_some() { " split" } else { "" });
        match beside {
            // Outside the shell's update, which the split's layout updates.
            Some(with) => {
                let window = self.window;
                cx.defer(move |cx| open_split(window, url, with, cx));
            }
            None => open_tab(self.window, url),
        }
    }

    fn site_search_settings(&self, _: &mut App) {
        open_tab(self.window, "chrome://settings/searchEngines");
    }

    fn clear_browsing_data(&self, _: &mut App) {
        open_tab(self.window, CLEAR_BROWSING_DATA);
    }

    fn close_other_window(&self, _: &mut App) -> bool {
        unsafe { platform::host_close_other_window() }
    }

    fn show_space(&self, space: u64, _: &mut App) {
        unsafe { bc::bc_window_show_space(self.window, space) };
    }

    fn space_tab(&self, space: u64) -> u64 {
        unsafe { bc::bc_window_space_tab(self.window, space) }
    }

    fn create_space(&self, name: &str, profile: u64, after: u64, _: &mut App) -> u64 {
        let space = create_space(name, after);
        if profile != 0 {
            unsafe { bc::bc_space_set_profile(space, profile) };
        }
        space
    }

    fn rename_space(&self, space: u64, name: &str, _: &mut App) {
        let name = CString::new(name).unwrap_or_default();
        unsafe { bc::bc_space_set_name(space, name.as_ptr()) };
    }

    fn rename_tab(&self, tab: u64, name: &str, _: &mut App) {
        let name = CString::new(name).unwrap_or_default();
        unsafe { bc::bc_tab_rename(tab, name.as_ptr()) };
    }

    fn set_icon(&self, tab: u64, icon: &str, _: &mut App) {
        let icon = CString::new(icon).unwrap_or_default();
        unsafe { bc::bc_tab_set_icon(tab, icon.as_ptr()) };
    }

    fn delete_space(&self, space: u64, _: &mut App) -> bool {
        unsafe { bc::bc_space_delete(space) }
    }

    fn set_space_color(&self, space: u64, color: Option<u32>, _: &mut App) {
        unsafe { bc::bc_space_set_color(space, color.is_some(), color.unwrap_or(0)) };
    }

    fn set_space_icon(&self, space: u64, icon: &str, _: &mut App) {
        let icon = CString::new(icon).unwrap_or_default();
        unsafe { bc::bc_space_set_icon(space, icon.as_ptr()) };
    }

    fn set_space_theme(&self, space: u64, theme: &str, _: &mut App) {
        let theme = CString::new(theme).unwrap_or_default();
        unsafe { bc::bc_space_set_theme(space, theme.as_ptr()) };
    }

    fn set_space_profile(&self, space: u64, profile: u64, _: &mut App) {
        unsafe { bc::bc_space_set_profile(space, profile) };
    }

    fn create_profile(&self, name: &str, _: &mut App) -> u64 {
        let name = CString::new(name).unwrap_or_default();
        unsafe { bc::bc_profile_create(name.as_ptr()) }
    }

    fn rename_profile(&self, profile: u64, name: &str, _: &mut App) {
        let name = CString::new(name).unwrap_or_default();
        unsafe { bc::bc_profile_rename(profile, name.as_ptr()) };
    }

    fn import_host(&self) -> Rc<dyn browser::import::Host> {
        Rc::new(import_host::CoreImport)
    }

    fn account_host(&self) -> Option<Rc<dyn browser::account::Host>> {
        // A build that names no server leaves the first launch's step out, as the address is for
        // Settings › Sync to ask (main's --sync-url).
        (!env!("SYNC_SERVER").is_empty()).then(settings::browser_account)
    }

    fn delete_profile(&self, profile: u64, _: &mut App) {
        unsafe { bc::bc_profile_delete(profile) };
    }

    fn context_menu(
        &self,
        items: Vec<MenuEntry>,
        at: Point<Pixels>,
        in_reveal: bool,
        look: menu::Look,
        chosen: Box<dyn FnOnce(Option<usize>, &mut App)>,
        cx: &mut App,
    ) {
        show_menu(self.ui_view, items, at, in_reveal, look, chosen, cx);
    }

    fn confirm(
        &self,
        message: String,
        detail: String,
        action: &'static str,
        confirmed: Box<dyn FnOnce(&mut App)>,
        cx: &mut App,
    ) {
        with_overlays(move |cx| dialog::ask(message, detail, action, confirmed, cx), cx);
    }

    fn tell(
        &self,
        question: dialog::Question,
        confirmed: Box<dyn FnOnce(Vec<String>, &mut App)>,
        cx: &mut App,
    ) -> bool {
        if !dialog::can_pose(cx) {
            return false;
        }
        with_overlays(move |cx| dialog::pose(question, confirmed, cx), cx);
        true
    }

    fn focus_ui(&self, _: &mut App) {
        focus_ui(self.window, self.view, false);
    }

    fn hold_keyboard_for_page(&self, _: &mut App) {
        focus_ui(self.window, self.view, true);
    }

    fn focus_page(&self, _: &mut App) {
        unsafe { bc::bc_window_focus_page(self.window) };
    }

    fn lay_out_page(&self, frame: Bounds<Pixels>, radius: f32, cx: &mut App) {
        set_page_frame(self.window, (frame, radius));
        // After the shell's update, which may be drawing its window.
        let window = self.window;
        cx.defer(move |cx| link_preview::page_moved(window, (frame, radius), cx));
    }

    fn set_focus_color(&self, color: u32, _: &mut App) {
        unsafe { bc::bc_window_set_focus_color(self.window, color) };
    }

    fn set_page_color(&self, color: u32, _: &mut App) {
        unsafe { bc::bc_window_set_page_color(self.window, color) };
    }

    fn set_window_light(&self, light: bool, _: &mut App) {
        unsafe { platform::host_set_window_light(self.ui_view, light) };
    }

    fn capture_look(&self, tab: u64, look: menu::Look, cx: &mut App) {
        capture::set_look(tab, look, cx);
    }

    fn set_appearance(&self, appearance: Appearance, _: &mut App) {
        settings::save_appearance(appearance);
        give_host_appearance(appearance);
    }

    fn page_moving(&self, moving: bool, _: &mut App) {
        unsafe { bc::bc_window_page_moving(self.window, moving) };
    }

    fn set_pane_corner(&self, right: f32, top: f32, _: &mut App) {
        unsafe { platform::host_set_pane_corner(self.ui_view, right as f64, top as f64) };
    }

    fn show_window_buttons(&self, shown: bool, _: &mut App) {
        unsafe { platform::host_show_window_buttons(self.ui_view, shown) };
    }

    #[cfg(target_os = "macos")]
    fn window_button_geometry(&self, _: &mut App) -> Option<browser::shell::WindowButtonGeometry> {
        let (mut centres, mut y, mut diameter) = ([0f64; 3], 0f64, 0f64);
        let read = unsafe {
            platform::host_window_buttons(self.ui_view, centres.as_mut_ptr(), &mut y, &mut diameter)
        };
        read.then(|| browser::shell::WindowButtonGeometry {
            centres: centres.map(|x| x as f32),
            y: y as f32,
            diameter: diameter as f32,
        })
    }

    fn press_window_button(&self, button: WindowButton, _: &mut App) {
        // Windows' own zoom button maximizes, and its window has no full screen of its own, so the
        // zoom Escape presses in full screen takes the window out through the core.
        if cfg!(target_os = "windows")
            && matches!(button, WindowButton::Zoom)
            && self.window_full_screen()
        {
            unsafe { bc::bc_window_toggle_full_screen(self.window) };
            return;
        }
        let button = match button {
            WindowButton::Close => 0,
            WindowButton::Minimize => 1,
            WindowButton::Zoom => 2,
        };
        unsafe { platform::host_press_window_button(self.ui_view, button) };
    }

    fn start_window_move(&self, _: &mut App) {
        // The core moves a window on Linux alone; on Windows the host does, from the app.
        #[cfg(target_os = "windows")]
        unsafe { platform::host_start_window_move(self.ui_view) };
        #[cfg(not(target_os = "windows"))]
        unsafe { bc::bc_window_start_move(self.window) };
    }

    fn window_maximized(&self) -> bool {
        #[cfg(target_os = "windows")]
        return unsafe { platform::host_window_maximized(self.ui_view) };
        #[cfg(not(target_os = "windows"))]
        return cfg!(target_os = "linux") && unsafe { bc::bc_window_is_maximized(self.window) };
    }

    fn window_full_screen(&self) -> bool {
        cfg!(any(target_os = "linux", target_os = "windows"))
            && unsafe { bc::bc_window_is_full_screen(self.window) }
    }

    fn show_reveal(&self, width: f32, _: &mut App) {
        unsafe { platform::host_show_reveal(self.ui_view, width.into()) };
    }

    fn hide_reveal(&self, _: &mut App) {
        unsafe { platform::host_hide_reveal(self.ui_view) };
    }

    fn option_reveals_sidebar(&self) -> bool {
        unsafe { bc::bc_option_reveals_sidebar() }
    }

    fn saved_sidebar_width(&self) -> Option<f32> {
        let width = unsafe { bc::bc_window_sidebar_width(self.window) };
        (width > 0.).then_some(width as f32)
    }

    fn set_sidebar_width(&self, width: f32, _: &mut App) {
        unsafe { bc::bc_window_set_sidebar_width(self.window, width.into()) };
    }

    fn detent(&self, _: &mut App) {
        if unsafe { bc::bc_haptics() } {
            unsafe { platform::host_detent() };
        }
    }

    fn watch_control(&self, _: &mut App) {
        unsafe { platform::host_watch_control(control_released) };
    }

    fn copy(&self, text: &str, cx: &mut App) {
        #[cfg(target_os = "linux")]
        system::copy(text, cx);
        #[cfg(not(target_os = "linux"))]
        {
            _ = cx;
            let text = CString::new(text.replace('\0', "")).unwrap_or_default();
            unsafe { platform::host_copy(text.as_ptr()) };
        }
    }

    fn pasted(&self, cx: &App) -> Option<String> {
        #[cfg(target_os = "linux")]
        return system::pasted(cx);
        #[cfg(not(target_os = "linux"))]
        {
            _ = cx;
            // Asked for its length first, then for the text.
            let length = unsafe { platform::host_paste(std::ptr::null_mut(), 0) };
            let mut text = vec![0u8; length.checked_add(1)?];
            unsafe { platform::host_paste(text.as_mut_ptr().cast(), text.len()) };
            text.truncate(length);
            Some(String::from_utf8_lossy(&text).into_owned()).filter(|text| !text.is_empty())
        }
    }

    fn can_open_in_safari(&self) -> bool {
        true
    }

    fn open_in_safari(&self, url: &str, _: &mut App) {
        if let Ok(url) = CString::new(url) {
            unsafe { platform::host_open_in_safari(url.as_ptr()) };
        }
    }

    fn can_use_system_engine(&self) -> bool {
        unsafe { bc::bc_system_engine_available() }
    }

    fn use_system_engine(&self, tab: u64, _: &mut App) {
        use_system_engine(tab);
    }

    fn close_media_note(&self, tab: u64, _: &mut App) {
        unsafe { bc::bc_tab_close_media_note(tab) };
    }

    fn widevine(&self) -> Widevine {
        Widevine::from_core(unsafe { bc::bc_widevine() })
    }

    fn download_widevine(&self, _: &mut App) {
        unsafe { bc::bc_widevine_download() };
    }

    fn restart(&self, cx: &mut App) {
        // As Quit does: once the shell that asked is done, and before Chromium's posted quit.
        cx.defer(save_launch_picture);
        unsafe { bc::bc_restart() };
    }

    fn playready_needs(&self) -> shell::PlayReady {
        shell::PlayReady::from_core(unsafe { bc::bc_playready_needs() })
    }

    fn share_services(&self, url: &str) -> Vec<SharedString> {
        unsafe extern "C" fn each(context: *mut c_void, name: *const c_char) {
            let names = unsafe { &mut *(context as *mut Vec<SharedString>) };
            names.push(unsafe { CStr::from_ptr(name) }.to_string_lossy().into_owned().into());
        }
        let mut names: Vec<SharedString> = Vec::new();
        if let Ok(url) = CString::new(url) {
            let context = &mut names as *mut Vec<SharedString> as *mut c_void;
            unsafe { platform::host_share_services(url.as_ptr(), each, context) };
        }
        names
    }

    fn share(&self, url: &str, title: &str, service: usize, _: &mut App) {
        if let (Ok(url), Ok(title)) = (CString::new(url), CString::new(title)) {
            unsafe { platform::host_share_with(url.as_ptr(), title.as_ptr(), service) };
        }
    }

    fn has_share_sheet(&self) -> bool {
        has_share_sheet()
    }

    fn show_share_sheet(&self, tab: u64, url: &str, title: &str, at: Point<Pixels>, cx: &mut App) {
        if let Some(window) = tab_window(tab, cx) {
            share_page(&window, url, title, Bounds::new(at, Size::default()));
        }
    }

    fn toast(&self, text: &str, key: Option<(&str, &str)>, time: Duration, cx: &mut App) {
        let text = text.to_string();
        let key = key.map(|(keys, what)| (keys.to_string(), what.to_string()));
        let show = move |cx: &mut App| {
            let key = key.as_ref().map(|(keys, what)| (keys.as_str(), what.as_str()));
            toast::show(&text, key, time, cx)
        };
        with_overlays(show, cx);
    }

    fn toast_undo(&self, text: &str, undo: Rc<dyn Fn(&mut App)>, time: Duration, cx: &mut App) {
        let text = text.to_string();
        with_overlays(move |cx| toast::show_undo(&text, undo, time, cx), cx);
    }

    fn hide_undo_toast(&self, cx: &mut App) {
        toast::hide_undo(cx);
    }

    fn in_front(&self) -> bool {
        front().0 == self.window
    }

    fn toast_button(
        &self,
        text: &str,
        (label, name): (Option<&'static str>, &'static str),
        run: Rc<dyn Fn(&mut App)>,
        time: Duration,
        cx: &mut App,
    ) {
        let text = text.to_string();
        let button = toast::Button {
            label: label.map_or(toast::Label::Plate, |label| toast::Label::Text(label.into())),
            name,
            run: Rc::new(move |_, cx| run(cx)),
        };
        with_overlays(move |cx| toast::show_with_buttons(&text, vec![button], time, cx), cx);
    }

    fn full_screen_note(&self, shown: bool, cx: &mut App) {
        match shown {
            true => with_overlays(toast::show_full_screen, cx),
            false => toast::hide_full_screen(cx),
        }
    }

    fn hang_note(&self, tab: u64, shown: bool, cx: &mut App) {
        if !shown {
            toast::hide_held(tab, cx);
            return;
        }
        let button = |label: &str, name, run: fn(u64)| toast::Button {
            label: toast::Label::Text(label.to_owned().into()),
            name,
            run: Rc::new(move |_, _| run(tab)),
        };
        let buttons = vec![
            button(shell::WAIT, "wait", |tab| unsafe { bc::bc_tab_wait(tab) }),
            button(shell::RELOAD, "reload", |tab| unsafe { bc::bc_tab_reload(tab, false) }),
        ];
        with_overlays(move |cx| toast::show_held(shell::NOT_RESPONDING, buttons, tab, cx), cx);
    }

    fn tooltip(&self, tip: Option<(tooltip::Tip, tooltip::Anchor)>, cx: &mut App) {
        match tip {
            Some((tip, anchor)) => tooltip::hover(tip, anchor, cx),
            None => tooltip::dismiss(cx),
        }
    }

    fn link_target(
        &self,
        plate: Option<(link_target::Plate, Bounds<Pixels>, link_target::Side)>,
        cx: &mut App,
    ) {
        match plate {
            Some((plate, frame, _)) => {
                let (left, top) = (frame.origin.x.into(), frame.origin.y.into());
                link_target::show(plate, left, top, cx)
            }
            None => link_target::hide(cx),
        }
    }

    fn command_bar(
        &self,
        editing: Option<(u64, SharedString)>,
        tabs: Vec<BarTab>,
        field: Option<Bounds<Pixels>>,
        cx: &mut App,
    ) {
        with_overlays(move |cx| command_bar::show(editing, tabs, field, cx), cx);
    }

    fn search_tabs(&self, tabs: Vec<BarTab>, cx: &mut App) {
        with_overlays(move |cx| command_bar::show_tabs(tabs, cx), cx);
    }

    fn command_bar_open(&self, cx: &App) -> bool {
        command_bar::is_open(cx)
    }

    fn command_bar_follows(
        &self,
        tabs: Vec<BarTab>,
        open_tabs: Vec<BarTab>,
        cx: &mut App,
    ) -> bool {
        // The bar shows over the window in front; another's Space, as an agent's, isn't its.
        self.window == front().0 && command_bar::space_shown(tabs, open_tabs, cx)
    }

    fn split_bar(&self, with: u64, pane: Bounds<Pixels>, card: Bounds<Pixels>, cx: &mut App) {
        with_overlays(move |cx| command_bar::show_split(with, pane, card, cx), cx);
    }

    fn find(&self, tab: u64, cx: &mut App) {
        let window = self.window;
        with_overlays(move |cx| find_bar::show(window, tab, cx), cx);
    }

    fn site_controls(
        &self,
        tab: u64,
        place: site_controls::Place,
        window: gpui::Size<Pixels>,
        extensions: Vec<site_controls::Extension>,
        cx: &mut App,
    ) {
        with_overlays(move |cx| site_controls::show(tab, place, window, extensions, cx), cx);
    }

    fn site_controls_extensions(
        &self,
        tab: u64,
        extensions: Vec<site_controls::Extension>,
        cx: &mut App,
    ) {
        // The panel isn't open while the overlays wait to open.
        site_controls::set_extensions(tab, extensions, cx);
    }

    fn capture(
        &self,
        tab: u64,
        frame: Bounds<Pixels>,
        radius: f32,
        full_page: bool,
        library: bool,
        look: menu::Look,
        cx: &mut App,
    ) {
        with_overlays(
            move |cx| match full_page {
                true => capture::full_page(tab, cx),
                false => capture::start(tab, frame, radius, library, look, cx),
            },
            cx,
        );
    }

    fn page_picture(&self, tab: u64, request: u64, _: &mut App) {
        // The page's whole view, which is the shape it has where it's live; the card fits it.
        unsafe { bc::bc_tab_picture(tab, request, 0, 0) };
    }

    fn hover_card(&self, hover: hover_card::Hover, cx: &mut App) {
        hover_card::hover(hover, cx);
    }

    fn folder_preview(&self, ask: folder_preview::Ask, cx: &mut App) {
        folder_preview::ask(ask, cx);
    }

    fn card_picture(&self, tab: u64, cx: &mut App) {
        let request = hover_card::asking(tab, cx);
        // check.sh reads this line: a hidden page draws for its picture from here.
        log!("browser: tab {tab} picturing {request}");
        let (width, height) = hover_card::PICTURE_PIXELS;
        unsafe { bc::bc_tab_picture(tab, request, width, height) };
    }

    fn window(&self) -> u64 {
        self.window
    }

    fn find_next(&self, tab: u64, forward: bool, cx: &mut App) {
        let window = self.window;
        with_overlays(move |cx| find_bar::find_next(window, tab, forward, cx), cx);
    }

    fn jump_to_selection(&self, tab: u64, _: &mut App) {
        if tab != 0 {
            unsafe { bc::bc_tab_jump_to_selection(tab) };
        }
    }

    fn settings(&self, cx: &mut App) {
        settings::show(cx);
    }

    fn shortcuts(&self, cx: &mut App) {
        settings::show_shortcuts(cx);
    }

    fn about(&self, cx: &mut App) {
        settings::show_about(cx);
    }

    fn updates(&self) -> bool {
        updates_available()
    }

    fn check_for_updates(&self, cx: &mut App) {
        check_for_updates(cx);
    }

    fn import(&self, cx: &mut App) {
        settings::show_import(cx);
    }

    fn passwords(&self, space: u64, cx: &mut App) {
        let profile = unsafe { bc::bc_space_profile(space) };
        settings::show_passwords(Some(profile).filter(|&profile| profile != 0), cx);
    }

    fn crash_reports(&self, _: &App) -> Option<bool> {
        let offered = !settings::crash_reports_collector().is_empty();
        offered.then(|| unsafe { bc::bc_send_crash_reports() })
    }

    fn set_crash_reports(&self, on: bool, cx: &mut App) {
        settings::set_send_crash_reports(on, cx);
    }

    // Every build has the agents' server. The first launch picks its steps before the first
    // frame, when agents_core makes the Agents, and a new profile's are off until then.
    fn agents(&self, cx: &App) -> Option<bool> {
        Some(browser::agents::global(cx).is_some_and(|agents| agents.read(cx).is_on()))
    }

    fn set_agents(&self, on: bool, cx: &mut App) {
        agents_core::start(cx);
        if let Some(agents) = browser::agents::global(cx) {
            agents.update(cx, |agents, cx| agents.set_on(on, cx));
        }
    }

    fn agent_activity(&self, cx: &mut App) {
        settings::show_agents(cx);
    }

    fn new_window(&self, cx: &mut App) {
        let id = self.window;
        // Outside the shell's update, as the new window's shell reads this one's.
        cx.defer(move |cx| {
            // From a private window, the Spaces are the regular window's in front.
            let from = WINDOWS.with_borrow(|windows| {
                let this = windows.iter().find(|w| w.id == id && !w.private);
                this.or_else(|| windows.iter().find(|w| !w.private)).cloned()
            });
            if from.is_some() {
                open_window(from, None, cx);
            }
        });
    }

    fn is_default_browser(&self) -> bool {
        unsafe { platform::host_is_default_browser() }
    }

    fn make_default_browser(&self, _: &mut App) {
        unsafe { platform::host_make_default_browser(settings::default_browser_answered) };
    }

    fn welcome_window(&self, cx: &mut App) {
        let id = self.window;
        cx.defer(move |cx| {
            let from = WINDOWS.with_borrow(|windows| {
                let this = windows.iter().find(|w| w.id == id && !w.private);
                this.or_else(|| windows.iter().find(|w| !w.private)).cloned()
            });
            let Some(window) = from.and_then(|from| open_window(Some(from), None, cx)) else {
                return;
            };
            let shell = WINDOWS.with_borrow(|windows| {
                windows.iter().find(|w| w.id == window).and_then(|w| w.shell.upgrade())
            });
            if let Some(shell) = shell {
                shell.update(cx, |shell, cx| shell.start_onboarding(None, cx));
            }
        });
    }

    fn new_private_window(&self, cx: &mut App) {
        let id = self.window;
        cx.defer(move |cx| {
            let from = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == id).cloned());
            // The profile of the Space this window shows, a private one's too.
            let profile = unsafe { bc::bc_space_profile(bc::bc_window_space(id)) };
            if from.is_some() && profile != 0 {
                open_window(from, Some(profile), cx);
            }
        });
    }

    fn close_window(&self, cx: &mut App) {
        let id = self.window;
        // Outside the shell's update: the core closes the NSWindow at once, and GPUI's handler for
        // that needs the App.
        cx.spawn(async move |_| {
            if WINDOWS.with_borrow(|windows| windows.iter().any(|w| w.id == id)) {
                unsafe { bc::bc_window_close(id) };
            }
        })
        .detach();
    }

    /// Asks first, as Arc does, unless the user said not to. Only the user's Quit asks: a quit from
    /// the Dock or at logout doesn't come here.
    fn quit(&self, cx: &mut App) {
        // Once the shell that asked is done, as it reads the shell. Chromium's quit is posted, so
        // comes after.
        cx.defer(save_launch_picture);
        // With no window to ask over, there's nothing open to lose.
        if !unsafe { bc::bc_warn_before_quitting() } || front().1.is_null() {
            return quit_because("the user's Quit, which had nothing to ask");
        }
        let question = dialog::Question {
            icon: None,
            message: concat!("Quit ", env!("APP_NAME"), "?").into(),
            detail: "".into(),
            list: Vec::new(),
            action: "Quit".into(),
            cancel: "Cancel".into(),
            tone: dialog::Tone::Accent,
            also: Some((
                "Quit, and don’t ask again".into(),
                Box::new(|_| {
                    // check.sh waits for this line.
                    log!("browser: warn before quitting off");
                    unsafe { bc::bc_set_warn_before_quitting(false) };
                    quit_because("the user's Quit, answered");
                }),
            )),
            fields: Vec::new(),
            check: None,
            on_return: dialog::Return::Action,
        };
        // A Quit as the window comes on screen, before its overlays are made, asks too (#905):
        // with no dialog to ask in, pose would drop it and the app would never quit.
        let quit = Box::new(|_: Vec<String>, _: &mut App| quit_because("the user's Quit, answered"));
        with_overlays(move |cx| dialog::pose(question, quit, cx), cx);
    }
}

/// Asks `message` in an alert with `buttons`, the first the default if `returns`, and returns the
/// index of the one chosen, or none if AppKit ended it. It blocks in AppKit's loop until the alert
/// closes, so it's called in a task. Linux has no alert, and its loop can't wait for one
/// (settings.rs).
#[cfg(not(target_os = "linux"))]
fn ask(message: &str, detail: &str, buttons: &[&str], returns: bool) -> usize {
    let text = |text: &str| CString::new(text).unwrap_or_default();
    let (message, detail) = (text(message), text(detail));
    let buttons: Vec<CString> = buttons.iter().map(|button| text(button)).collect();
    let pointers: Vec<*const c_char> = buttons.iter().map(|button| button.as_ptr()).collect();
    let chosen = unsafe {
        platform::host_ask(
            message.as_ptr(),
            detail.as_ptr(),
            pointers.as_ptr(),
            pointers.len(),
            returns,
        )
    };
    usize::try_from(chosen).unwrap_or(usize::MAX)
}

/// A key the page didn't keep, or one it never sees, as a keystroke in GPUI's terms.
fn keystroke(key: &bc::bc_key) -> String {
    let text = |text: *const c_char| match text.is_null() {
        true => Default::default(),
        false => unsafe { CStr::from_ptr(text) }.to_string_lossy(),
    };
    shell::page_keystroke(&text(key.key), &text(key.latin_key), key.modifiers)
}

/// Runs the shortcut `key` is, if it's one of the page shortcuts that are `reserved` or aren't,
/// on the keys the user has now, in the window showing `tab`'s page, which had the key.
fn run_page_shortcut(tab: bc::bc_id, key: *const bc::bc_key, reserved: bool) -> bool {
    let key = unsafe { &*key };
    let keystroke = keystroke(key);
    // Read without an update, as every key the page gets comes here.
    let cx = CX.with_borrow(Clone::clone);
    let command = cx.and_then(|cx| {
        cx.try_read_global(|keys: &shell::UserKeys, _| keys.command_for(&keystroke, reserved))
    });
    let Some(command) = command.flatten() else {
        return false;
    };
    // ⌘Z and ⇧⌘Z in a text field are the page's, which the Edit menu gives it.
    if matches!(command, Command::Undo | Command::Redo) && key.editing {
        return false;
    }
    // Its field opens before GPUI has read the keys typed after it, which it holds for the field
    // under Wayland, as for a shortcut the page gets first (gpui_linux::hold_keys).
    #[cfg(target_os = "linux")]
    if reserved && command.opens_field() {
        gpui_linux::hold_keys(Some(&held_keystroke(key)));
    }
    update_tab_window(tab, |shell, cx| shell.run_for_page(command, cx)).unwrap_or(false)
}

thread_local! {
    /// The shortcut a page got first last, which a second press runs ilo's of if the page kept it.
    static SECOND_PRESS: RefCell<shell::SecondPress> = RefCell::default();
}

unsafe extern "C" fn tab_key_before_page(
    _: *mut c_void,
    tab: bc::bc_id,
    key: *const bc::bc_key,
) -> bool {
    // An agent's key comes from inside the Agents' update, where an update of ours would abort.
    if browser::agents::pressing_key() {
        return false;
    }
    #[cfg(target_os = "linux")]
    if held_escape::keeps_from_page(unsafe { &*key }) {
        return true;
    }
    // Any key ends the hover card, as in Dia, the page's as the UI's.
    update_app(hover_card::close);
    // The page has the keyboard, so the sidebar's ring of Focus Sidebar has gone stale.
    update_tab_window(tab, |shell, cx| shell.leave_keys("page", cx));
    if let Some(lost) = link_preview_key(tab, key, true) {
        return lost;
    }
    // As in Chrome, the reserved shortcuts come first, then the extensions' keys.
    if run_page_shortcut(tab, key, true) {
        return true;
    }
    let keystroke = keystroke(unsafe { &*key });
    // ⌘. is the agent bar's Stop while it shows one, before the page could keep it.
    if update_tab_window(tab, |shell, cx| shell.run_agent_key(&keystroke, cx)).unwrap_or(false) {
        return true;
    }
    if update_tab_window(tab, |shell, cx| shell.run_extension_key(&keystroke, "page", cx))
        .unwrap_or(false)
    {
        return true;
    }
    let cx = CX.with_borrow(Clone::clone);
    // A shortcut the page gets first that it kept, pressed again at once, runs ilo's (Arc's).
    let shortcut = cx.as_ref().and_then(|cx| {
        cx.try_read_global(|keys: &shell::UserKeys, _| keys.command_for(&keystroke, false))
    });
    let repeat = unsafe { platform::host_key_repeated() };
    let now = Instant::now();
    let shortcut = shortcut.flatten().is_some();
    if SECOND_PRESS.with_borrow_mut(|second| second.press(tab, &keystroke, shortcut, repeat, now)) {
        // check.sh reads this line.
        log!("browser: second press {keystroke}");
        return run_page_shortcut(tab, key, false);
    }
    // The keys typed after ⌘L would reach the page before it hands ⌘L back to open the bar.
    let holds = cx.and_then(|cx| {
        cx.try_read_global(|keys: &shell::UserKeys, _| keys.holds_keys(&keystroke))
    });
    if holds == Some(true) {
        // The core says when the page has answered the key and no field opens for it, and holds
        // them itself where the host can't (bc_hold_keys).
        unsafe { bc::bc_hold_keys(cfg!(target_os = "linux")) };
        unsafe { platform::host_hold_keys() };
        #[cfg(target_os = "linux")]
        gpui_linux::hold_keys(Some(&held_keystroke(unsafe { &*key })));
    }
    false
}

/// The keystroke GPUI has for `key`, one typed in a page.
#[cfg(target_os = "linux")]
fn held_keystroke(key: &bc::bc_key) -> gpui::Keystroke {
    let name = unsafe { CStr::from_ptr(key.key) }.to_string_lossy();
    let has = |flag| key.modifiers & flag != 0;
    let modifiers = gpui::Modifiers {
        control: has(bc::BC_MODIFIER_CONTROL),
        alt: has(bc::BC_MODIFIER_ALT),
        shift: has(bc::BC_MODIFIER_SHIFT),
        platform: has(bc::BC_MODIFIER_META),
        function: false,
    };
    shell::web_keystroke(&name, modifiers)
}

/// Gives the keys the core held from a page since a shortcut that opens a field (bc_hold_keys) to
/// the overlay of `view`, which now has the keyboard, in the order typed.
#[cfg(target_os = "linux")]
fn give_held_keys(view: *mut c_void) {
    unsafe extern "C" fn take(held: *mut c_void, keys: *const bc::bc_key, count: usize) {
        let held = unsafe { &mut *held.cast::<Vec<gpui::Keystroke>>() };
        for key in unsafe { std::slice::from_raw_parts(keys, count) } {
            held.push(held_keystroke(key));
        }
    }
    // Under Wayland GPUI has every key too and gives the overlay its own, those after the
    // shortcut's press, which both had; the core's go to the page only if nothing takes them.
    if gpui_linux::give_held_keys(view) {
        unsafe { bc::bc_give_held_keys(None, std::ptr::null_mut()) };
        return;
    }
    let mut held: Vec<gpui::Keystroke> = Vec::new();
    unsafe { bc::bc_give_held_keys(Some(take), (&raw mut held).cast()) };
    let Some(cx) = CX.with_borrow(Clone::clone).filter(|_| !held.is_empty()) else {
        return;
    };
    // check.sh reads this line, which names the window that took them as the Mac's does.
    log!("browser: held {} key events for GPUIPanel", held.len());
    // After the update that showed the overlay, which a key would come inside, and before GPUI
    // reads the keys typed since, as the task runs before it dispatches its Wayland queue.
    let app = cx.clone();
    cx.foreground_executor()
        .spawn(async move {
            app.update(|cx| {
                let overlay = cx.windows().into_iter().find(|window| {
                    window.update(cx, |_, window, _| ns_view(window) == view).unwrap_or(false)
                });
                if let Some(overlay) = overlay {
                    let typed = |_: gpui::AnyView, window: &mut Window, cx: &mut App| {
                        for keystroke in held {
                            window.dispatch_keystroke(keystroke, cx);
                        }
                    };
                    overlay.update(cx, typed).ok();
                }
            })
        })
        .detach();
}

/// No field opens for the key the keys after it are held for (bc_hold_keys), so the host gives them
/// to the page.
unsafe extern "C" fn held_keys_unclaimed(_: *mut c_void) {
    unsafe { platform::host_release_held_keys() };
}

unsafe extern "C" fn tab_key_unhandled(
    _: *mut c_void,
    tab: bc::bc_id,
    key: *const bc::bc_key,
) -> bool {
    if browser::agents::pressing_key() {
        return false;
    }
    SECOND_PRESS.with_borrow_mut(|second| second.declined(&keystroke(unsafe { &*key })));
    link_preview_key(tab, key, false).unwrap_or_else(|| run_page_shortcut(tab, key, false))
}

/// Opens a browser window and returns its id, with `ui_view` filling it under the page: the first
/// at the frame the front window had when the browser last quit, saved window `saved` behind the
/// others at its own (bc_window_open_saved), each kept on screen, else centered on the main
/// display, or cascaded from the one of `after` and as big. With `private`, a profile, it's a
/// private window in that profile's off-the-record session.
fn open_browser_window(
    ui_view: *mut c_void,
    after: Option<*mut c_void>,
    private: Option<bc::bc_id>,
    saved: Option<usize>,
) -> Option<bc::bc_id> {
    let frames = if after.is_none() || saved.is_some() { saved_frames() } else { Vec::new() };
    let frame = frames.get(saved.unwrap_or(0)).copied().and_then(placed);
    let bounds = frame.unwrap_or_else(|| {
        let (mut width, mut height) = WINDOW_SIZE;
        let (mut x, mut y) = (0., 0.);
        match after {
            Some(after) => unsafe {
                platform::host_cascaded(after, &mut x, &mut y, &mut width, &mut height)
            },
            None => unsafe { platform::host_centered(width, height, &mut x, &mut y) },
        }
        bc::bc_rect {
            x,
            y,
            width,
            height,
        }
    });
    let window = match (private, saved) {
        (Some(profile), _) => unsafe { bc::bc_window_open_private(bounds, ui_view, profile) },
        (None, Some(index)) => unsafe { bc::bc_window_open_saved(index, bounds, ui_view) },
        (None, None) => unsafe { bc::bc_window_open(bounds, ui_view) },
    };
    (window != 0).then_some(window)
}

/// The frames of the windows open when the browser last quit, front to back.
fn saved_frames() -> Vec<bc::bc_rect> {
    let count = unsafe { bc::bc_saved_window_frames(std::ptr::null_mut(), 0) };
    let mut frames = vec![
        bc::bc_rect {
            x: 0.,
            y: 0.,
            width: 0.,
            height: 0.,
        };
        count
    ];
    let count = unsafe { bc::bc_saved_window_frames(frames.as_mut_ptr(), frames.len()) };
    frames.truncate(count);
    frames
}

/// Where a window saved at `saved` opens on the screens there are now, or None for the app's own
/// place.
fn placed(saved: bc::bc_rect) -> Option<bc::bc_rect> {
    let count = unsafe { platform::host_screens(std::ptr::null_mut(), 0) };
    let mut screens = vec![0.; count * 4];
    let count = unsafe { platform::host_screens(screens.as_mut_ptr(), count) };
    let screens: Vec<Frame> = screens
        .as_chunks::<4>()
        .0
        .iter()
        .take(count)
        .map(|&[x, y, width, height]| Frame { x, y, width, height })
        .collect();
    let saved = Frame { x: saved.x, y: saved.y, width: saved.width, height: saved.height };
    let frame = window_frames::place(saved, &screens)?;
    Some(bc::bc_rect {
        x: frame.x,
        y: frame.y,
        width: frame.width,
        height: frame.height,
    })
}

/// The front window's frame, kept beside its launch picture, for the next launch's window to open
/// at before the core can say where the browser window will.
fn launch_frame_path() -> String {
    format!("{}/launch-frame", profile_dir())
}

/// Opens the windows after the first that were open when the browser last quit, each behind the
/// one before it, as they were.
fn open_saved_windows(cx: &mut App) {
    for index in 1..saved_frames().len() {
        // The window it goes behind, which it starts from as ⌘N's window does.
        let Some(above) = WINDOWS.with_borrow(|windows| windows.last().cloned()) else {
            return;
        };
        open_window_as(Some(above), None, Some(index), cx);
    }
}

/// Shows the Spaces as the user left them in `window`, else a first Space, and says whether it
/// brought them back. A page given in BROWSER_URL, and the links from other apps that launched the
/// browser, open on top either way, in this window, the one in front. With nothing to bring back
/// and nothing to open, it's a new profile's first run: `shell` plays the onboarding, then shows
/// the welcome on the empty card. This is its only way in at launch, so a launch that brings Spaces
/// back never reads more than `restored` for it. check.sh sets BROWSER_ONBOARDING to "skip" for
/// every run but the onboarding run's, which sets "light" or "dark".
fn restore_spaces(window: bc::bc_id, shell: WindowHandle<Shell>, cx: &mut App) -> bool {
    let start = Instant::now();
    let restored = unsafe { bc::bc_window_restore(window) };
    if restored {
        // check.sh reads this line.
        log!(
            "browser: restored in {:.1} ms",
            start.elapsed().as_secs_f64() * 1000.
        );
    } else {
        unsafe { bc::bc_window_show_space(window, create_space("Space 1", 0)) };
    }
    // The rules name the Spaces by their places, so they're read once the Spaces are back.
    link_rules_host::read();
    let links = LAUNCH_LINKS.take().unwrap_or_default();
    match std::env::var("BROWSER_URL") {
        Ok(url) => open_tab(window, &url),
        Err(_) if !restored && links.is_empty() => {
            let onboarding = std::env::var("BROWSER_ONBOARDING").ok();
            shell
                .update(cx, |shell, _, cx| {
                    shell.set_welcome(cx);
                    if onboarding.as_deref() != Some("skip") {
                        shell.start_onboarding(onboarding.as_deref(), cx);
                    }
                })
                .ok();
        }
        Err(_) => {}
    }
    open_links_in(window, &links);
    restored
}

/// Logs where the window is, for check.sh.
fn log_frame(id: bc::bc_id, ui_view: *mut c_void) {
    let (mut x, mut y, mut width, mut height) = (0., 0., 0., 0.);
    if unsafe { platform::host_window_frame(ui_view, &mut x, &mut y, &mut width, &mut height) } {
        log!("browser: window {id} frame {x:.0} {y:.0} {width:.0} {height:.0}");
    }
}

/// Links another app sent: a link clicked in Mail, `open https://…`, an .html file from Finder.
/// Each opens as a new tab in the window in front, as a link to a new tab does, and that window
/// comes to the front. Those that launched the browser wait for its first window.
unsafe extern "C" fn open_links(_: *mut c_void, urls: *const *const c_char, count: usize) {
    let urls: Vec<String> = unsafe { std::slice::from_raw_parts(urls, count) }
        .iter()
        .map(|&url| unsafe { CStr::from_ptr(url) }.to_string_lossy().into_owned())
        .collect();
    let launching = LAUNCH_LINKS.with_borrow_mut(|links| {
        links.as_mut().map(|links| links.extend(urls.iter().cloned())).is_some()
    });
    if launching {
        return;
    }
    // A private window's session is the user's on purpose, so a link from elsewhere goes to the
    // regular window in front, as Chrome's does.
    let Some(window) = WINDOWS.with_borrow(|windows| windows.iter().find(|w| !w.private).cloned())
    else {
        return;
    };
    let shown = open_links_in(window.id, &urls);
    let ui_view = WINDOWS.with_borrow(|windows| {
        windows.iter().find(|w| w.id == shown).map_or(window.ui_view, |w| w.ui_view)
    });
    unsafe { platform::host_activate(ui_view) };
    // AppKit hands over the links that launched the browser all at once, so the first window
    // needn't wait for it to finish.
    finished_launching();
}

/// Opens a tab loading `url` in the front window and shows it: in the Space the window shows if
/// it's one of the user's in `profile`, else in the first of theirs that is, which the window then
/// shows, as a profile's own page, such as Chrome's password manager, shows that profile's data.
/// With none, in the Space shown, unless it's an agent's, which none of the user's tabs go in.
fn open_in_profile(profile: bc::bc_id, url: &str) {
    let window = front().0;
    if window == 0 {
        return;
    }
    let users = |space: bc::bc_id| space != 0 && name(bc::bc_space_agent, space).is_empty();
    let theirs =
        |space: bc::bc_id| users(space) && unsafe { bc::bc_space_profile(space) } == profile;
    let shown = unsafe { bc::bc_window_space(window) };
    let space = match theirs(shown) {
        true => Some(shown),
        false => ids(bc::bc_spaces).into_iter().find(|&space| theirs(space)),
    };
    let Some(space) = space.or(users(shown).then_some(shown)) else {
        return;
    };
    if space != shown {
        unsafe { bc::bc_window_show_space(window, space) };
    }
    open_tab(window, url);
}

/// Opens Chrome's Clear browsing data dialog for `profile`, as Arc's Settings › Profiles does, in
/// the regular window in front, showing a Space of the profile first if it shows another's. Returns
/// whether it did: not for a profile none of the user's Spaces is in, as its tabs would have none.
pub fn clear_browsing_data(profile: u64) -> bool {
    let Some(window) = WINDOWS.with_borrow(|windows| windows.iter().find(|w| !w.private).cloned())
    else {
        return false;
    };
    let users = |space: bc::bc_id| {
        let theirs = unsafe { bc::bc_space_profile(space) } == profile;
        theirs && name(bc::bc_space_agent, space).is_empty()
    };
    let shown = unsafe { bc::bc_window_space(window.id) };
    let Some(space) = std::iter::once(shown).chain(ids(bc::bc_spaces)).find(|&space| users(space))
    else {
        return false;
    };
    if space != shown {
        unsafe { bc::bc_window_show_space(window.id, space) };
    }
    open_tab(window.id, CLEAR_BROWSING_DATA);
    unsafe { platform::host_activate(window.ui_view) };
    // check.sh reads this line.
    log!("browser: clear browsing data of profile {profile} in space {space}");
    true
}

/// Opens `urls` as new tabs in `front`, the regular window in front, in the Space it shows, or in
/// the user's it showed last if that's an agent's, which none of the user's tabs go in. The last
/// one opened shows, and the window it's in is returned.
fn open_links_in(front: bc::bc_id, urls: &[String]) -> bc::bc_id {
    let mut window = front;
    for url in urls {
        let (shown, tab, space) = open_link_in(front, url, link_rules_host::route(url));
        window = shown;
        if space == 0 {
            continue;
        }
        // check.sh reads this line.
        log!("browser: link from another app {url}: tab {tab} in space {space} of window {window}");
    }
    window
}

/// Opens `url` as a new tab, shown, in the Space `routed`, a link rule's, in the regular window
/// showing it, else in `front` switched to it; with `routed` 0, in `front`'s user's Space. Returns
/// the window, the tab and the Space, 0 if there's no Space for it.
fn open_link_in(front: bc::bc_id, url: &str, routed: bc::bc_id) -> (bc::bc_id, u64, bc::bc_id) {
    let showing = (routed != 0).then(|| {
        WINDOWS.with_borrow(|windows| {
            let shows = |w: &&BrowserWindow| unsafe { bc::bc_window_space(w.id) } == routed;
            windows.iter().filter(|w| !w.private).find(shows).map(|w| w.id)
        })
    });
    let window = showing.flatten().unwrap_or(front);
    let space = match routed {
        0 => users_space_of(window),
        routed => routed,
    };
    if space == 0 {
        return (window, 0, 0);
    }
    if space != unsafe { bc::bc_window_space(window) } {
        unsafe { bc::bc_window_show_space(window, space) };
    }
    let tab = open_tab_behind(window, url, false);
    if tab != 0 {
        unsafe { bc::bc_window_select(window, tab) };
    }
    (window, tab, space)
}

/// The Space `window` shows, or the user's it showed last if that's an agent's, else the user's
/// first, or 0.
fn users_space_of(window: bc::bc_id) -> bc::bc_id {
    let users = |space: bc::bc_id| space != 0 && name(bc::bc_space_agent, space).is_empty();
    let shown = unsafe { bc::bc_window_space(window) };
    if users(shown) {
        return shown;
    }
    let last = WINDOWS.with_borrow(|windows| {
        windows.iter().find(|w| w.id == window).map_or(0, |w| w.users_space)
    });
    let spaces = ids(bc::bc_spaces);
    match spaces.contains(&last) && users(last) {
        true => last,
        false => spaces.into_iter().find(|&space| users(space)).unwrap_or(0),
    }
}

// The ⌃Tab walk and the file's drag were the user's in the window in front.
extern "C" fn control_released() {
    update_window(front().0, |shell, cx| shell.control_released(cx));
}

extern "C" fn file_preview(
    download: u64,
    png: *const u8,
    size: usize,
    width: f64,
    height: f64,
    image: bool,
) {
    let preview = (!png.is_null()).then(|| {
        let png = unsafe { std::slice::from_raw_parts(png, size) }.to_vec();
        let png = Arc::new(Image::from_bytes(ImageFormat::Png, png));
        FilePreview { image: png, width: width as f32, height: height as f32, photo: image }
    });
    let kind = match preview {
        Some(_) if image => "picture",
        Some(_) => "icon",
        None => "none",
    };
    log!("browser: download {download} preview {kind} {width}x{height}");
    update_shells(|shell, cx| shell.set_download_preview(download, preview.clone(), cx));
}

extern "C" fn file_drag_ended(taken: bool) {
    update_window(front().0, |shell, cx| shell.file_drag_ended(taken, cx));
}

extern "C" fn dragged(
    ui_view: *mut c_void,
    phase: c_int,
    x: f64,
    y: f64,
    items: *const platform::host_dragged,
    count: usize,
) -> bool {
    let items = match items.is_null() {
        true => &[][..],
        false => unsafe { std::slice::from_raw_parts(items, count) },
    };
    let items: Vec<_> = items
        .iter()
        .map(|item| (unsafe { CStr::from_ptr(item.url) }.to_string_lossy(), item.file))
        .collect();
    let items = items.iter().map(|(url, file)| (url.as_ref(), *file));
    let window = window_with_view(ui_view);
    let search = |text: &str| {
        url_for(text, |text, url, capacity| unsafe {
            bc::bc_search_url(window, std::ptr::null(), text, url, capacity)
        })
    };
    let urls = shell::dropped_urls(items, search);
    let position = point(px(x as f32), px(y as f32));
    let answer = update_window(window, |shell, cx| match phase {
        platform::DRAG_OVER => !urls.is_empty() && shell.drag_in(position, cx),
        platform::DRAG_DROP => shell.drop_in(position, &urls, cx),
        _ => {
            shell.drag_away(cx);
            false
        }
    });
    answer.unwrap_or(false)
}

/// Opens a browser window after `id`'s, for ⇧↩ in the command bar: as ⌘N does, or from a private
/// window as ⇧⌘N does, so what was typed there stays off the record. Returns it.
fn open_window_after(id: bc::bc_id, cx: &mut App) -> Option<bc::bc_id> {
    let from = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == id).cloned())?;
    // The profile of the Space a private window shows, as ⇧⌘N takes it.
    let private = from.private.then(|| unsafe { bc::bc_space_profile(bc::bc_window_space(id)) });
    if private == Some(0) {
        return None;
    }
    open_window(Some(from), private, cx)
}

/// Opens a browser window and the shell inside it under the page, and returns it. The first opens
/// the overlays every window shares and the Spaces as the user left them. Another, from `from`,
/// starts with its shell's state and shows its Space with no page, as ⌘N's window does. With
/// `private`, a profile, it's ⇧⌘N's private window, which shows a Space of its own instead, with
/// no page.
fn open_window(
    from: Option<BrowserWindow>,
    private: Option<bc::bc_id>,
    cx: &mut App,
) -> Option<bc::bc_id> {
    open_window_as(from, private, None, cx)
}

/// open_window, or with `saved`, the saved window after the first the launch brings back
/// (bc_window_open_saved): behind `from` and not key, starting with its shell's state as ⌘N's
/// window does, but showing the Space it showed, with its page.
fn open_window_as(
    from: Option<BrowserWindow>,
    private: Option<bc::bc_id>,
    saved: Option<usize>,
    cx: &mut App,
) -> Option<bc::bc_id> {
    #[cfg(not(target_os = "linux"))]
    let ui_view = unsafe { platform::host_ui_view() };
    // GPUI's own X11 window, which its window opens in, as the view the core holds.
    #[cfg(target_os = "linux")]
    let ui_view = gpui_linux::new_host_window().unwrap_or(0) as usize as *mut c_void;
    let after = from.as_ref().map(|from| from.ui_view);
    let id = open_browser_window(ui_view, after, private, saved)?;
    // Under Wayland, the window's surface, which GPUI's goes in as a subsurface, is the view, and
    // GPUI's window starts at its size.
    #[cfg(target_os = "linux")]
    let (ui_view, wayland_size) = {
        let (mut width, mut height) = (0., 0.);
        let surface = unsafe { bc::bc_window_wayland_surface(id, &mut width, &mut height) };
        match surface.is_null() {
            true => (ui_view, None),
            false => (surface, Some(size(px(width as f32), px(height as f32)))),
        }
    };
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    unsafe { platform::host_window_opened(ui_view, id) };
    let from_shell = from.as_ref().and_then(|from| from.shell.upgrade());
    // Its Space, which the core made with the window.
    let private_space = private.map(|profile| (unsafe { bc::bc_window_space(id) }, profile));
    // The shell reads its sidebar's width from the core as it's made. A saved window's is its own.
    if let Some(from) = from.as_ref().filter(|_| saved.is_none()) {
        let width = unsafe { bc::bc_window_sidebar_width(from.id) };
        unsafe { bc::bc_window_set_sidebar_width(id, width) };
    }
    #[cfg(target_os = "macos")]
    gpui_macos::open_next_window_in(ui_view);
    #[cfg(target_os = "windows")]
    gpui_windows::open_next_window_in(ui_view);
    #[cfg(target_os = "linux")]
    match wayland_size {
        Some(_) => gpui_linux::open_next_window_in_surface(ui_view, 0, 0),
        None => gpui_linux::open_next_window_in(ui_view as usize as u32),
    }
    // Transparent, over the window's material.
    let options = WindowOptions {
        window_background: WindowBackgroundAppearance::Transparent,
        #[cfg(target_os = "linux")]
        window_bounds: wayland_size
            .map(|size| gpui::WindowBounds::Windowed(Bounds::new(point(px(0.), px(0.)), size))),
        ..Default::default()
    };
    let window = cx.open_window(options, |window, cx| {
        let shell = cx.new(|cx| {
            // The page keeps its place in the card as the window resizes.
            cx.observe_window_bounds(window, move |shell: &mut Shell, window, cx| {
                let viewport = window.viewport_size();
                // First, as a page's full screen can end with the window's.
                #[cfg(any(target_os = "linux", target_os = "windows"))]
                shell.bounds_changed(cx);
                if let Some(frame) = shell.window_page_frame(viewport) {
                    set_frames(id, shell, frame, viewport);
                    shell.full_screen_step("laid", viewport);
                }
                // The panels keep their place from the page's top right corner, and the dialog
                // its insets from the content's edges. A menu closes as the window resizes, as the
                // Mac's does.
                #[cfg(target_os = "linux")]
                {
                    panels::place(ui_view);
                    overlays::place_dialog(ui_view);
                    menu_window::resized(ui_view);
                    popovers::resized(ui_view);
                    link_preview_window::resized(ui_view);
                    reveal_window::place(ui_view);
                }
            })
            .detach();
            #[cfg(target_os = "linux")]
            system::watch_activation(window, ui_view, wayland_size.is_some(), cx);
            // Once GPUI's window is in the UI's, where Chromium's own drags may go.
            #[cfg(target_os = "linux")]
            if wayland_size.is_none() {
                drops::take(ui_view);
            }
            let engine = Rc::new(Engine {
                window: id,
                ui_view,
                view: ns_view(window),
                videos: RefCell::default(),
            });
            match (&from_shell, private_space) {
                (_, Some((space, profile))) => {
                    Shell::private_window(engine, space, private_name(profile), profile, cx)
                }
                (Some(from), None) if saved.is_some() => Shell::seeded_behind(from, engine, cx),
                (Some(from), None) => Shell::seeded_from(from, engine, cx),
                (None, None) => Shell::new(engine, cx),
            }
        });
        window.focus(&shell.focus_handle(cx), cx);
        CX.set(Some(cx.to_async()));
        if let Some(state) = SYNC_STATE.with_borrow_mut(Option::take) {
            settings::sync_changed(state, cx);
        }
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        emoji::keep(cx);
        // In front, as it's about to be key, or a saved one behind the others.
        WINDOWS.with_borrow_mut(|windows| {
            windows.insert(
                if saved.is_some() { windows.len() } else { 0 },
                BrowserWindow {
                    id,
                    ui_view,
                    shell: shell.downgrade(),
                    handle: Window::window_handle(window),
                    children: Vec::new(),
                    private: private.is_some(),
                    users_space: 0,
                },
            )
        });
        if from_shell.is_none() {
            // The core read the extensions before there was a shell to take them.
            unsafe { bc::bc_extensions_report() };
        }
        shell
    });
    let window = match window {
        Ok(window) => window,
        Err(error) => {
            log!("couldn't open the window: {error}");
            if from.is_none() {
                quit_because("the first window didn't open");
            } else {
                unsafe { bc::bc_window_close(id) };
            }
            return None;
        }
    };
    window
        .update(cx, |shell, window, _| {
            let viewport = window.viewport_size();
            set_frames(id, shell, shell.page_frame(viewport), viewport)
        })
        .ok();
    if let Some(from) = from.as_ref().filter(|_| private.is_none()) {
        // A saved window shows the Space the core put back in it.
        let space = if saved.is_some() {
            unsafe { bc::bc_window_space(id) }
        } else {
            let space = unsafe { bc::bc_window_space(from.id) };
            unsafe { bc::bc_window_show_space_empty(id, space) };
            space
        };
        // Its Space drawn before it's on screen, rather than once the core's report comes.
        window.update(cx, |shell, _, cx| shell.set_space(space, cx)).ok();
    }
    let first = from.is_none();
    // Once the page frame is set, so the tab shown loads at its size, and before the window is on
    // screen, so the core's reports of its tabs can come before its first frame.
    let restored = first && restore_spaces(id, window, cx);
    match (&from, saved) {
        (Some(from), Some(_)) => unsafe { platform::host_show_window_behind(ui_view, from.ui_view) },
        _ => unsafe { platform::host_show_window(ui_view) },
    }
    log_frame(id, ui_view);
    if !first {
        finish_opening(id, ui_view, window, true, saved.is_none(), private, cx);
        return Some(id);
    }
    // check.sh reads this line.
    log!("browser: window on screen at {:.1} ms", unsafe { platform::host_since_launch() });
    // Making the overlays' windows takes the main thread for a while, so at launch they wait for a
    // turn of their own after the frame that follows the first. That frame draws the sidebar the
    // core has reported by then and takes the launch picture down, so it goes on screen first. A
    // shortcut that wants an overlay before makes them at once (with_overlays). Later windows
    // don't wait, as a page may open in them at once. A window that isn't visible (another
    // desktop, launched hidden) draws no frames, so a timer makes them if the frames haven't.
    let open = move |cx: &mut App| {
        finish_opening(id, ui_view, window, false, false, private, cx);
        // After the overlays, so the window the user sees first waits for neither.
        if restored {
            open_saved_windows(cx);
        }
        // Last, as nothing the user sees at launch waits for it.
        start_updates(cx);
    };
    OVERLAYS.set(Some(Box::new(open)));
    let pending = Rc::new(Cell::new(true));
    let finish = {
        let pending = pending.clone();
        move |cx: &mut App| {
            if !pending.replace(false) {
                return;
            }
            // Once the frame this precedes is drawn.
            cx.spawn(async move |cx| {
                unsafe { platform::host_close_launch_window() };
                // check.sh reads this line: the UI comes out from under the launch picture.
                log!("browser: UI shown at {:.1} ms", unsafe { platform::host_since_launch() });
                cx.update(overlays_now);
                cx.update(agents_core::start)
            })
            .detach();
        }
    };
    let on_frame = finish.clone();
    let handle = window;
    window
        .update(cx, |_, window, _| {
            window.on_next_frame(move |window, _| {
                // check.sh reads this line.
                log!("browser: first frame at {:.1} ms", unsafe { platform::host_since_launch() });
                // Deferred, as the shell's window is borrowed while it draws.
                let next = move |window: &mut Window| {
                    window.on_next_frame(move |_, cx| cx.defer(on_frame))
                };
                // The picture stays over the sidebar until a frame drawn once AppKit has handed
                // over the links that launched the browser, so the restored tab never shows
                // selected first. A plain launch has heard by now or within a few ms, before the
                // next frame, so it doesn't wait.
                match FINISHED_LAUNCHING.get() {
                    true => next(window),
                    false => AFTER_LAUNCH.set(Some(Box::new(move |cx| {
                        handle.update(cx, |_, window, _| next(window)).ok();
                    }))),
                }
            })
        })
        .ok();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_millis(500)).await;
        cx.update(finish)
    })
    .detach();
    Some(id)
}

thread_local! {
    /// AppKit has finished launching the app, and so has handed over the links that launched it.
    static FINISHED_LAUNCHING: Cell<bool> = const { Cell::new(false) };
    /// What waits for it: the first window's frame that takes the launch picture down.
    static AFTER_LAUNCH: RefCell<Option<Box<dyn FnOnce(&mut App)>>> = const { RefCell::new(None) };
    /// The overlays' windows at launch, made on the turn after the frame that takes the launch
    /// picture down, as making them holds the main thread a while.
    static OVERLAYS: RefCell<Option<Box<dyn FnOnce(&mut App)>>> = const { RefCell::new(None) };
}

/// Makes the overlays' windows now if they're still to come.
fn overlays_now(cx: &mut App) {
    if let Some(open) = OVERLAYS.take() {
        open(cx);
    }
}

/// Shows an overlay, making the overlays' windows first if a shortcut asks for one before they're
/// made. Then at the end of the update, as making them reads the shell, which is in one.
fn with_overlays(show: impl FnOnce(&mut App) + 'static, cx: &mut App) {
    match OVERLAYS.with_borrow(Option::is_some) {
        true => cx.defer(move |cx| {
            // check.sh reads this line.
            log!("browser: overlays opened for a shortcut");
            overlays_now(cx);
            show(cx);
        }),
        false => show(cx),
    }
}

/// The window's last steps: its own overlays, and the first's those every window shares. With
/// `seeded`, it's a later window, whose keyboard is the UI's with `ui_keyboard`, as one without a
/// page.
fn finish_opening(
    id: bc::bc_id,
    ui_view: *mut c_void,
    window: WindowHandle<Shell>,
    seeded: bool,
    ui_keyboard: bool,
    private: Option<bc::bc_id>,
    cx: &mut App,
) {
    let Ok(shell) = window.entity(cx) else {
        return;
    };
    let weak = shell.downgrade();
    let look = move |cx: &App| {
        let shell = weak.upgrade()?;
        Some((shell.read(cx).window_fill(), shell.read(cx).palette()))
    };
    let prompt = offer::open(
        id,
        Rc::new(OfferHost { ui_view }),
        move |cx| look(cx).unwrap_or_default(),
        cx,
    );
    if let Some(view) = prompt.and_then(|prompt| prompt.update(cx, |_, w, _| ns_view(w)).ok()) {
        let (right, top) = (offer::RIGHT.into(), offer::TOP.into());
        unsafe { platform::host_add_offer(ui_view, view, right, top) };
        // The core can report the window's tab before this, so the offer learns it here.
        let selected = shell.read(cx).selected();
        offer::tab_shown(id, selected, cx);
    }
    let children: Vec<AnyWindowHandle> = [
        find_bar::open(id, ui_view, cx),
        prompt.map(Into::into),
        reveal::open(shell.clone(), ui_view, cx),
    ]
    .into_iter()
    .flatten()
    .collect();
    WINDOWS.with_borrow_mut(|windows| {
        if let Some(window) = windows.iter_mut().find(|window| window.id == id) {
            window.children = children;
        }
    });
    // check.sh reads these lines.
    log!("browser: window opened {id}");
    if let Some(profile) = private {
        log!("browser: private window opened {id} profile {profile}");
        // The core showed its Space before there was a window to title.
        shell.update(cx, |shell, cx| shell.update_title(cx));
    }
    if seeded {
        // With no page, the keyboard is the UI's, as in a Space without tabs.
        if ui_keyboard && let Ok(view) = window.update(cx, |_, window, _| ns_view(window)) {
            focus_ui(id, view, false);
        }
        return;
    }
    unsafe {
        platform::host_take_drops(dragged);
        platform::host_watch_key_window(key_window_changed);
        platform::host_watch_full_screen(full_screen_changed);
        platform::host_watch_swipes(swiped);
        platform::host_watch_option_taps(option_tapped);
        platform::host_watch_resigning_active(launch_picture_on_resigning);
        platform::host_watch_active(became_active);
    }
    follow_reduce_motion(cx);
    #[cfg(not(target_os = "linux"))]
    unsafe { platform::host_watch_reduce_motion(reduce_motion_changed) };
    #[cfg(target_os = "linux")]
    reduce_motion::watch(cx);
    if let Some(bar) = command_bar::open(Rc::new(Bar), cx)
        && let Ok(view) = bar.update(cx, |_, window, _| ns_view(window))
    {
        let radius = command_bar::RADIUS.into();
        elevate(COMMAND_BAR, Level::Modal);
        unsafe { platform::host_add_overlay(COMMAND_BAR, view, radius, command_bar_dismissed) };
    }
    if let Some(picker) = icon_picker::open(Rc::new(PickerHost), recent_emoji(), cx)
        && let Ok(view) = picker.update(cx, |_, window, _| ns_view(window))
    {
        let corners: Vec<f64> = icon_picker::corners()
            .iter()
            .flat_map(|&(x, y, radius)| [x, y, radius].map(f64::from))
            .collect();
        elevate(ICON_PICKER, Level::Modal);
        unsafe {
            platform::host_add_overlay(ICON_PICKER, view, 0., icon_picker_dismissed);
            platform::host_set_overlay_shape(ICON_PICKER, corners.as_ptr(), corners.len() / 3);
            let (x, y) = icon_picker::ARROW_TIP;
            platform::host_set_overlay_growth(ICON_PICKER, x.into(), y.into());
        }
    }
    if let Some(preview) = folder_preview::open(Rc::new(FolderPreviewHost), cx)
        && let Ok(view) = preview.update(cx, |_, window, _| ns_view(window))
    {
        let radius = folder_preview::RADIUS.into();
        elevate(FOLDER_PREVIEW, Level::Popover);
        let dismissed = folder_preview_dismissed;
        unsafe { platform::host_add_overlay(FOLDER_PREVIEW, view, radius, dismissed) };
    }
    if let Some(picker) = theme_picker::open(Rc::new(ThemeHost), cx)
        && let Ok(view) = picker.update(cx, |_, window, _| ns_view(window))
    {
        elevate(THEME_PICKER, Level::Modal);
        unsafe {
            platform::host_add_overlay(THEME_PICKER, view, 0., theme_picker_dismissed);
            platform::host_set_overlay_shrink(THEME_PICKER);
        }
    }
    if let Some(dialog) = dialog::open(Rc::new(DialogHost), cx)
        && let Ok(view) = dialog.update(cx, |_, window, _| ns_view(window))
    {
        unsafe { platform::host_add_dialog(view) };
    }
    // The window the toast shows over gives its colour and palette.
    let look = |cx: &App| {
        let shell = WINDOWS.with_borrow(|windows| windows.first().map(|w| w.shell.clone()));
        let shell = shell?.upgrade()?;
        Some((shell.read(cx).window_fill(), shell.read(cx).palette()))
    };
    let toast_host = Rc::new(ToastHost { view: Cell::new(std::ptr::null_mut()) });
    if let Some(toast) =
        toast::open(toast_host.clone(), move |cx| look(cx).unwrap_or_default(), cx)
        && let Ok(view) = toast.update(cx, |_, window, _| ns_view(window))
    {
        toast_host.view.set(view);
        unsafe { platform::host_add_toast(view) };
        // Auto-archive at launch can come before there's a toast to show its note in.
        show_archive_note(cx);
    }
    if let Some(capture) = capture::open(Rc::new(CaptureHost { toast: toast_host }), cx)
        && let Ok(view) = capture.update(cx, |_, window, _| ns_view(window))
    {
        unsafe { platform::host_add_capture(view, capture_dismissed) };
    }
    let host = Rc::new(LinkPreviewHost {
        holder: Cell::new(std::ptr::null_mut()),
        over: Cell::new(0),
    });
    if let Some(preview) = link_preview::open(host.clone(), cx)
        && let Ok(view) = preview.update(cx, |_, window, _| ns_view(window))
    {
        let holder = unsafe { platform::host_add_link_preview(view, link_preview_dismissed) };
        host.holder.set(holder);
        LINK_PREVIEWS.set(!holder.is_null());
    }
    let host = Rc::new(SiteControlsHost { view: Cell::new(std::ptr::null_mut()) });
    if let Some(panel) = site_controls::open(host.clone(), cx)
        && let Ok(view) = panel.update(cx, |_, window, _| ns_view(window))
    {
        host.view.set(view);
        elevate(SITE_CONTROLS, Level::Popover);
        unsafe { platform::host_add_site_controls(view, site_controls_dismissed) };
    }
    if let Some(menu) = menu::open(Rc::new(MenuHost), cx)
        && let Ok(view) = menu.update(cx, |_, window, _| ns_view(window))
    {
        unsafe { platform::host_add_menu_window(view, menu_pointer, menu_dismissed, menu_choose) };
    }
    // Their windows reach this far past the plate, for its shadow.
    let margin = f64::from(browser::elevation::WINDOW_MARGIN);
    if let Some(tip) = tooltip::open(Rc::new(TooltipHost), cx)
        && let Ok(view) = tip.update(cx, |_, window, _| fading_view(window))
    {
        unsafe { platform::host_add_tooltip(view, margin, tooltip_away) };
    }
    if let Some(card) = hover_card::open(Rc::new(CardHost), cx)
        && let Ok(view) = card.update(cx, |_, window, _| ns_view(window))
    {
        unsafe {
            platform::host_add_hover_card(view, margin, card_away);
            platform::host_watch_memory_pressure(memory_pressed);
        }
    }
    if let Some(target) =
        link_target::open(Rc::new(LinkTargetHost), move |cx| look(cx).unwrap_or_default(), cx)
        && let Ok(view) = target.update(cx, |_, window, _| fading_view(window))
    {
        unsafe { platform::host_add_link_target(view, margin, link_pointer) };
    }
    if let Some(hud) = zoom_hud::open(Rc::new(HudHost), cx)
        && let Ok(view) = hud.update(cx, |_, window, _| ns_view(window))
    {
        let (right, top) = (zoom_hud::WINDOW_RIGHT, zoom_hud::WINDOW_TOP);
        let margin = f64::from(zoom_hud::MARGIN);
        unsafe { platform::host_add_hud(view, f64::from(right), f64::from(top), margin) };
    }
    drop_targets::open(cx);
}

/// Gives an overlay's shadow window its level's shadow, before the overlay is added.
fn elevate(overlay: c_int, level: Level) {
    let shadow = level.host_shadow();
    unsafe {
        platform::host_set_overlay_elevation(
            overlay,
            shadow.outline_opacity,
            shadow.outline_radius,
            shadow.shadow_opacity,
            shadow.shadow_radius,
            shadow.shadow_drop,
        )
    };
}

unsafe extern "C" fn started(_: *mut c_void) {
    // Chromium's message loop dispatches GPUI's messages, as its run loop does on the Mac.
    #[cfg(target_os = "windows")]
    gpui_windows::embed();
    // Under Wayland GPUI joins Chromium's display, and Chromium dispatches GPUI's events after its
    // own, as what it reads can include them. On X11 GPUI has a connection of its own.
    #[cfg(target_os = "linux")]
    let wayland = {
        extern "C" fn dispatched() {
            gpui_linux::dispatch_embedded_after_read();
        }
        // Every key reaches the browser window and the UI alike there, and the core says whose
        // they are, as the X server does by its focus.
        fn window_in(surface: *mut c_void) -> Option<bc::bc_id> {
            // GPUI's dispatch can run inside a call to the core made while the list changes.
            WINDOWS.with(|windows| {
                let windows = windows.try_borrow().ok()?;
                windows.iter().find(|w| w.ui_view == surface).map(|w| w.id)
            })
        }
        let keyboard = gpui_linux::EmbeddedKeyboard {
            pressed: |surface| {
                if let Some(window) = window_in(surface) {
                    unsafe { bc::bc_window_ui_takes_keyboard(window) };
                }
            },
            is_ui: |surface| {
                window_in(surface).is_some_and(|window| unsafe { bc::bc_window_keyboard_is_ui(window) })
            },
            text_input: |surface, caret| {
                if let Some(window) = window_in(surface) {
                    let caret = caret.map(|caret| {
                        (f32::from(caret.origin.x), f32::from(caret.origin.y), f32::from(caret.size.height))
                    });
                    let (x, y, height) = caret.unwrap_or_default();
                    unsafe {
                        bc::bc_window_ui_text_input(window, caret.is_some(), x as f64, y as f64, height as f64)
                    };
                }
            },
        };
        let display = unsafe { bc::bc_wayland_display(Some(dispatched)) };
        match display.is_null() {
            // GPUI's X11 scale is its own guess otherwise, from Xft.dpi, where Chromium also
            // follows GDK_SCALE.
            true => {
                // A click in the UI takes the keyboard there, to keep, as Wayland's does: moving
                // X's input focus into GPUI's window tells the core nothing while it's there,
                // as after ⌘W, where the UI holds the keys for the page shown next (#1209).
                fn pressed(ui_view: *mut c_void) {
                    if let Some(window) = window_in(ui_view) {
                        unsafe { bc::bc_window_ui_takes_keyboard(window) };
                    }
                }
                gpui_linux::embed_x11(unsafe { bc::bc_display_scale() } as f32, pressed);
                // The clipboard's owner can be Chromium, which answers on this thread, so the UI's
                // read has it answer while it waits (#999).
                gpui_linux::answer_clipboard_reads_with(|| unsafe {
                    bc::bc_answer_clipboard_requests()
                });
            }
            false => {
                gpui_linux::embed_wayland(display, keyboard);
                gpui_linux::watch_embedder_swipes(system::swiped);
            }
        }
        !display.is_null()
    };
    let application = gpui_platform::application().with_assets(shell::Assets);
    // On X11, as Chromium is, even with a Wayland display in the environment.
    #[cfg(target_os = "linux")]
    let application = match wayland {
        true => application,
        false => application.with_windowing(gpui::WindowingRequest::Windowed(
            gpui::GraphicalEnvironment {
                wayland_display: None,
                ..gpui::GraphicalEnvironment::detect()
            },
        )),
    };
    let app = application.run_embedded(move |cx| {
        browser::face::register(cx);
        // The user's keys, before the shortcuts are bound to them.
        settings::read_shortcuts(cx);
        settings::read_appearance(cx);
        give_host_appearance(shell::app_appearance(cx));
        shell::bind_keys(cx);
        browser::text_input::bind_keys(cx);
        #[cfg(target_os = "linux")]
        if !wayland {
            held_escape::watch(cx);
        }
        cx.set_global(menu::field_menu(Rc::new(|window, request, cx| {
            // A browser window's field by the view the host knows the window by, as its other
            // menus are, which on Linux isn't GPUI's own: Chromium's surface or the X window GPUI
            // opened in. An overlay's by its own.
            let browser = WINDOWS.with_borrow(|windows| {
                windows.iter().find(|each| each.handle == window).map(|each| each.ui_view)
            });
            let view = browser
                .map_or_else(|| cx.update_window(window, |_, window, _| ns_view(window)), Ok);
            if let Ok(view) = view {
                let menu::Request { items, at, look, chosen, .. } = request;
                show_menu(view, items, at, false, look, chosen, cx);
            }
        })));
        cx.on_action(|_: &Hide, cx| cx.hide());
        cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
        cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
        cx.on_action(|_: &Minimize, _| unsafe {
            platform::host_press_window_button(front().1, 1)
        });
        cx.on_action(|_: &CheckForUpdates, cx| check_for_updates(cx));
        cx.bind_keys([
            KeyBinding::new("cmd-h", Hide, Some(shell::KEY_CONTEXT)),
            KeyBinding::new("alt-cmd-h", HideOthers, Some(shell::KEY_CONTEXT)),
            KeyBinding::new("cmd-m", Minimize, Some(shell::KEY_CONTEXT)),
        ]);
        cx.set_menus(menus(&shell::MenuBar::default(), None, None));
        // check.sh reads this line, as launches are timed from it.
        log!("browser: started at {:.1} ms", unsafe { platform::host_since_launch() });
        dock_dot::open(Rc::new(DockDot), cx);
        open_window(None, None, cx);
    });
    // On Chromium's Wayland display, GPUI's windows and their surfaces are on a display Chromium
    // has closed by the time the thread-locals go at exit, and dropping them there crashes (#474).
    #[cfg(target_os = "linux")]
    if wayland {
        std::mem::forget(app);
    } else {
        APP.set(Some(app));
    }
    #[cfg(not(target_os = "linux"))]
    APP.set(Some(app));
    #[cfg(target_os = "linux")]
    if let Some(fd) = gpui_linux::embedded_fd() {
        extern "C" fn ready() {
            gpui_linux::dispatch_embedded();
        }
        unsafe { platform::host_watch_fd(fd, ready) };
    }
}

/// AppKit has finished launching the app, and so has handed over the links that launched it.
extern "C" fn finished_launching() {
    if !FINISHED_LAUNCHING.replace(true)
        && let Some(then) = AFTER_LAUNCH.take()
    {
        update_app(then);
    }
}

/// Whether this build updates itself, asked once, as the menu bar is rebuilt often: a release
/// build with its updater, or on Linux the Flatpak.
fn updates_available() -> bool {
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    #[cfg(target_os = "linux")]
    let available = flatpak::in_flatpak;
    #[cfg(not(target_os = "linux"))]
    let available = || unsafe { platform::host_updates_available() };
    *AVAILABLE.get_or_init(available)
}

/// Starts checking for updates: Sparkle or WinSparkle, or on Linux Flatpak's portal.
fn start_updates(cx: &mut App) {
    #[cfg(not(target_os = "linux"))]
    {
        _ = cx;
        // The installer WinSparkle started waits for the browser to quit, which needs no asking.
        extern "C" fn quit() {
            quit_because("the updater's install");
        }
        unsafe { platform::host_start_updater(quit) };
    }
    #[cfg(target_os = "linux")]
    if updates_available() {
        cx.spawn(async move |cx| {
            let (portal, events) = match flatpak::Portal::open_session().await {
                Ok(opened) => opened,
                Err(error) => {
                    log!("browser: no updates: {error}");
                    let why = format!("Flatpak’s portal didn’t answer: {error}");
                    return cx.update(|cx| flatpak::changed(flatpak::State::Failed(why), cx));
                }
            };
            log!("browser: updater started");
            let restarting = portal.clone();
            let retries = cx.update(|cx| {
                flatpak::on_restart(move |cx| restart_into_newest(restarting.clone(), cx), cx);
                flatpak::changed(flatpak::State::Current, cx);
                flatpak::retries(cx)
            });
            flatpak::watch(&portal, events, retries, flatpak::ANSWER, |state| {
                log!("browser: update {state:?}");
                cx.update(|cx| flatpak::changed(state.clone(), cx));
            })
            .await;
        })
        .detach();
    }
}

/// The app menu's and the command bar's Check for Updates…: Sparkle's or WinSparkle's, or on Linux
/// where the Flatpak's update stands.
fn check_for_updates(cx: &mut App) {
    #[cfg(not(target_os = "linux"))]
    {
        _ = cx;
        unsafe { platform::host_check_for_updates() };
    }
    #[cfg(target_os = "linux")]
    flatpak::check(cx);
}

/// Starts the build Flatpak installed, once this one has let go of the profile, and quits.
#[cfg(target_os = "linux")]
fn restart_into_newest(portal: flatpak::Portal, cx: &mut App) {
    let lock = std::path::Path::new(&profile_dir()).join("SingletonLock");
    cx.spawn(async move |_| match portal.restart(&lock).await {
        Ok(()) => quit_because("the restart into the newest build"),
        Err(error) => log!("browser: no restart: {error}"),
    })
    .detach();
}

/// The menu bar: Arc's menus, with the commands the browser has, what `bar` says of the key
/// window, its Spaces, its Space's profile's extensions and the titles that follow its state, and
/// the titles Undo and Redo have for what it would undo and redo (docs/ux-spec.md › Menus). The
/// Edit menu's standard items go to the first responder, so a focused page cuts, copies, pastes,
/// selects and undoes for itself.
fn menus(bar: &shell::MenuBar, undo: Option<&str>, redo: Option<&str>) -> Vec<Menu> {
    let menu = |name: &str, items| Menu { name: name.to_string().into(), items, disabled: false };
    let item = shell::menu_item;
    let (spaces, extensions) = (&bar.spaces, &bar.extensions);
    let about = item(concat!("About ", env!("APP_NAME")), Command::About);
    let mut app = vec![about, MenuItem::separator()];
    // Only a release build that carries Sparkle has updates to check for.
    if updates_available() {
        app.extend([MenuItem::action("Check for Updates…", CheckForUpdates), MenuItem::separator()]);
    }
    app.extend([
        item("Settings…", Command::Settings),
        item("Import from Other Browsers…", Command::Import),
        item("Privacy Policy", Command::PrivacyPolicy),
        MenuItem::separator(),
        MenuItem::os_submenu("Services", SystemMenuType::Services),
        MenuItem::separator(),
        MenuItem::action(concat!("Hide ", env!("APP_NAME")), Hide),
        MenuItem::action("Hide Others", HideOthers),
        MenuItem::action("Show All", ShowAll),
        MenuItem::separator(),
        item(concat!("Quit ", env!("APP_NAME")), Command::Quit),
    ]);
    vec![
        menu(env!("APP_NAME"), app),
        menu(
            "File",
            vec![
                item("New Tab…", Command::NewTab),
                item("New Window", Command::NewWindow),
                item("New Incognito Window", Command::NewPrivateWindow),
                item("Restore Last Closed Tab", Command::ReopenClosedTab),
                MenuItem::separator(),
                item("Open Command Bar", Command::EditUrl),
                MenuItem::separator(),
                item("Close Window", Command::CloseWindow),
                item("Archive Tab", Command::CloseTab),
                MenuItem::separator(),
                item("Share…", Command::Share),
                item("Capture…", Command::CapturePortion),
                item("Capture Full Page", Command::CaptureFullPage),
                item("Save Page As", Command::SavePage),
                item("Print", Command::Print),
            ],
        ),
        menu(
            "Edit",
            vec![
                MenuItem::os_action(edit_title("Undo", undo), Undo, OsAction::Undo),
                MenuItem::os_action(edit_title("Redo", redo), Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action("Cut", Cut, OsAction::Cut),
                MenuItem::os_action("Copy", Copy, OsAction::Copy),
                item("Copy URL", Command::CopyUrl),
                item("Copy URL as Markdown", Command::CopyMarkdownLink),
                MenuItem::os_action("Paste", Paste, OsAction::Paste),
                MenuItem::os_action(
                    "Paste and Match Style",
                    PasteAndMatchStyle,
                    OsAction::PasteAndMatchStyle,
                ),
                item("Paste URL as New Tab", Command::PasteUrlAsTab),
                MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
                MenuItem::separator(),
                MenuItem::submenu(menu(
                    "Find",
                    vec![
                        item("Find…", Command::Find),
                        item("Find Next", Command::FindNext),
                        item("Find Previous", Command::FindPrevious),
                        item("Jump to Selection", Command::JumpToSelection),
                    ],
                )),
            ],
        ),
        menu(
            "View",
            vec![
                MenuItem::submenu(menu(
                    "Appearance",
                    (Appearance::ALL.into_iter())
                        .map(|each| {
                            item(appearance_title(each), Command::Appearance(each))
                                .checked(bar.appearance == each)
                        })
                        .collect(),
                )),
                MenuItem::separator(),
                item(bar.sidebar_title(), Command::ToggleSidebar),
                item(bar.pinned_item().0, bar.pinned_item().1).disabled(!bar.pinned_offered),
                MenuItem::separator(),
                item("Stop Loading", Command::Stop),
                item("Refresh the Page", Command::Reload),
                item("Force Refresh the Page", Command::ForceReload),
                item("Clear Cookies and Refresh", Command::ClearCookiesAndRefresh),
                item("Clear Cache and Refresh", Command::ClearCacheAndRefresh),
                MenuItem::separator(),
                item("Add Split View", Command::AddSplit),
                item("Close this Split Pane", Command::CloseSplitPane),
                item("Separate Page from Split View", Command::SeparatePane),
                MenuItem::separator(),
                item("Zoom to Actual Size", Command::ActualSize),
                item("Zoom In", Command::ZoomIn),
                item("Zoom Out", Command::ZoomOut),
                MenuItem::separator(),
                item("Cast…", Command::Cast),
                MenuItem::separator(),
                MenuItem::submenu(menu(
                    "Developer",
                    vec![
                        item("View Source", Command::ViewSource),
                        item("Developer Tools", Command::DevTools),
                        item("Inspect Elements", Command::InspectElements),
                        item("JavaScript Console", Command::JavaScriptConsole),
                    ]
                    .into_iter()
                    .chain(bar.developer_mode_title().into_iter().flat_map(|title| {
                        [MenuItem::separator(), item(title, Command::ToggleDeveloperMode)]
                    }))
                    .collect(),
                )),
                MenuItem::separator(),
                item("Expand Current Split", Command::ExpandSplit),
            ],
        ),
        menu(
            "Spaces",
            [
                item("New Space…", Command::NewSpace),
                item("Rename Space", Command::RenameSpace),
            ]
            .into_iter()
            .chain(bar.profiles.as_ref().map(change_profile))
            .chain([
                MenuItem::separator(),
                item("Next Space", Command::NextSpace),
                item("Previous Space", Command::PreviousSpace),
            ])
            .chain((!spaces.is_empty()).then(MenuItem::separator))
            .chain(spaces.iter().enumerate().map(|(index, name)| {
                shell::menu_item(name.clone(), Command::SelectSpace(index))
            }))
            .collect(),
        ),
        menu(
            "Tabs",
            vec![
                item("New Tab…", Command::NewTab),
                item("Search Tabs…", Command::SearchTabs),
                item(bar.pin_title(), Command::TogglePin),
                item("New Folder…", Command::NewFolder),
                MenuItem::separator(),
                item("Next Tab", Command::NextTab),
                item("Previous Tab", Command::PreviousTab),
                item("Reveal Tab in Sidebar", Command::RevealTab),
                item("Focus Sidebar", Command::FocusSidebar),
                MenuItem::separator(),
                item("Clear Today", Command::Clear),
                item("Reset all tabs in this Space", Command::ResetSpaceTabs),
            ],
        ),
        menu(
            "Archive",
            vec![
                item("Go Back", Command::Back),
                item("Go Forward", Command::Forward),
                MenuItem::separator(),
                item("View History", Command::ViewHistory),
                item("View Archive…", Command::ViewArchive),
                item("Clear Archive", Command::ClearArchive),
                MenuItem::separator(),
                item("Clear Browsing Data…", Command::ClearBrowsingData),
            ],
        ),
        menu(
            "Extensions",
            (extensions.iter().map(shell::extension_menu_item))
                .chain((!extensions.is_empty()).then(MenuItem::separator))
                .chain((!extensions.is_empty()).then(|| {
                    let pins = extensions.iter().map(shell::extension_pin_menu_item).collect();
                    MenuItem::submenu(menu("Pin to URL Field", pins))
                }))
                .chain([
                    item("Add Extension…", Command::AddExtension),
                    item("Manage Extensions…", Command::ManageExtensions),
                ])
                .collect(),
        ),
        menu(
            "Window",
            vec![
                MenuItem::action("Minimize Window", Minimize),
                MenuItem::separator(),
                item("Open Library…", Command::ViewLibrary),
                item("View Downloads…", Command::ViewDownloads),
                item("View Media…", Command::ViewMedia),
            ],
        ),
        menu(
            "Help",
            vec![
                item(concat!("Welcome to ", env!("APP_NAME")), Command::Welcome),
                item("Help Center", Command::HelpCenter),
                item("What's New", Command::WhatsNew),
                item("Essential Keyboard Shortcuts", Command::KeyboardShortcuts),
                MenuItem::separator(),
                item("Report a Problem…", Command::ReportProblem),
                MenuItem::submenu(menu(
                    "Troubleshooting",
                    vec![
                        item("Copy Build Info", Command::CopyBuildInfo),
                        item("Open Task Manager", Command::TaskManager),
                        item(bar.trace_title(), Command::RecordTrace),
                    ],
                )),
            ],
        ),
    ]
}

/// View › Appearance's item for `appearance`, as Arc's names them.
fn appearance_title(appearance: Appearance) -> &'static str {
    match appearance {
        Appearance::Automatic => "Automatic",
        Appearance::Light => "Light",
        Appearance::Dark => "Dark",
    }
}

/// Gives the host View › Appearance's choice, for the app's native parts and its pages.
fn give_host_appearance(appearance: Appearance) {
    let code = match appearance {
        Appearance::Automatic => 0,
        Appearance::Light => 1,
        Appearance::Dark => 2,
    };
    unsafe { platform::host_set_app_appearance(code) };
    // The Mac's pages follow NSApp's appearance, which the host sets.
    #[cfg(not(target_os = "macos"))]
    unsafe { bc::bc_set_appearance(code) };
}

/// Spaces › Change Profile: the profiles the Space shown can be in, as its menu's Profile items
/// are, and New Profile.
fn change_profile((choices, new): &(Vec<shell::ProfileChoice>, bool)) -> MenuItem {
    let items = (choices.iter())
        .map(|choice| {
            shell::menu_item(choice.name.clone(), Command::ChangeProfile(Some(choice.profile)))
                .checked(choice.checked)
                .disabled(!choice.enabled)
        })
        .chain([
            MenuItem::separator(),
            shell::menu_item("New Profile", Command::ChangeProfile(None)).disabled(!new),
        ])
        .collect();
    MenuItem::submenu(Menu { name: "Change Profile".into(), items, disabled: false })
}

// Closing the last window quits, as its close button does in Chrome's last window.
unsafe extern "C" fn window_closed(_: *mut c_void, id: bc::bc_id) {
    let (window, left) = WINDOWS.with_borrow_mut(|windows| {
        let index = windows.iter().position(|window| window.id == id);
        (index.map(|index| windows.remove(index)), windows.len())
    });
    let private = window.as_ref().is_some_and(|window| window.private);
    let regular = WINDOWS.with_borrow(|windows| windows.iter().any(|window| !window.private));
    if let Some(window) = window {
        // The user closing the last regular window, which doesn't resign the app, sets where the
        // next launch's window opens. Not in a quit, which closes the windows in no order once the
        // user's Quit has taken the one in front.
        if !private && !regular && !unsafe { bc::bc_quitting() } {
            save_launch_frame(window.ui_view);
        }
        unsafe { platform::host_forget_window(window.ui_view) };
        update_app(|cx| {
            find_bar::window_closed(id, cx);
            offer::window_closed(id, cx);
            for child in window.children {
                child.update(cx, |_, window, _| window.remove_window()).ok();
            }
            // The shared overlays and the menu bar follow the window now in front.
            if let Some(front) = WINDOWS.with_borrow(|windows| windows.first().cloned()) {
                front.shell.update(cx, |shell, cx| shell.became_key(cx)).ok();
            }
        });
    }
    log!("browser: window closed {id}");
    // A private window alone doesn't keep the browser: the Spaces' pages closed with the last
    // regular window, which the next launch brings back, so the private ones close with them.
    if left == 0 || (!private && !regular) {
        quit_because("the last window closed");
    }
}

/// Quits the browser, saying what asked, as each way the browser ends logs a line that names it:
/// this, Chromium's shutdown (main_delegate.cc) and, on the Mac, a quit Apple Event (host.mm).
fn quit_because(why: &str) {
    // check.sh reads this line.
    log!("browser: quit for {why}");
    unsafe { bc::bc_quit() };
}

unsafe extern "C" fn private_session_ended(_: *mut c_void, profile: bc::bc_id) {
    // check.sh reads this line.
    log!("browser: private session ended {profile}");
}

/// An agent's action as the core judged it, for the activity log in Settings › Agents, which
/// check.sh reads in the log too. One it let the agent do puts the agent's dot on the tab's row,
/// from the same entry. It comes from inside the call that asked, so the app hears it after.
unsafe extern "C" fn agent_activity(_: *mut c_void, entry: *const bc::bc_agent_activity) {
    let entry = unsafe { &*entry };
    let text = |text| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();
    let (agent, action, refused) = (text(entry.agent), text(entry.action), text(entry.refused));
    let tab = entry.tab;
    match refused.is_empty() {
        true => log!("browser: agent activity {agent} {tab} {action}"),
        false => log!("browser: agent activity {agent} {tab} {action} refused: {refused}"),
    }
    if refused.is_empty() {
        let agent = SharedString::from(agent.clone());
        update_app_later(move |cx| update_shells_in(cx, |shell, cx| {
            shell.agent_acted(agent.clone(), tab, cx)
        }));
    }
    // The Agents' own calls log themselves.
    if !agents_core::calling() {
        agents_core::update(move |agents, cx| agents.checked(&agent, tab, &action, &refused, cx));
    }
}

/// The core ended an agent's session with a tab: its client disconnected, it released the tab, the
/// user took the tab back or its page asked for the user's passkey. The activity log has it beside
/// what the agent did, and the agent is no longer at work in the tab.
unsafe extern "C" fn agent_session_ended(
    _: *mut c_void,
    agent: *const c_char,
    tab: bc::bc_id,
    why: c_int,
) {
    let agent = unsafe { CStr::from_ptr(agent) }.to_string_lossy().into_owned();
    let why = match why as u32 {
        bc::BC_AGENT_SESSION_DISCONNECTED => "disconnected",
        bc::BC_AGENT_SESSION_RELEASED => "released",
        bc::BC_AGENT_SESSION_TAKEN_BACK => "taken back",
        bc::BC_AGENT_SESSION_PASSKEY => "passkey",
        _ => "ended",
    };
    // check.sh reads this line.
    log!("browser: agent session ended {agent} {tab} {why}");
    // As agent_activity, from inside the call that ended it.
    update_app_later(move |cx| {
        update_shells_in(cx, |shell, cx| shell.agent_session_ended(&agent, tab, cx));
        if let Some(agents) = browser::agents::global(cx) {
            agents.update(cx, |agents, cx| agents.session_ended(&agent, tab, why, cx));
        }
    });
}

/// The core's answer to an agent's action in a tab (bc_agent_page): why it couldn't, a
/// screenshot, or the page's tree.
unsafe extern "C" fn agent_page_done(
    _: *mut c_void,
    tab: bc::bc_id,
    request: u64,
    text: *const c_char,
    error: bool,
) {
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();
    // check.sh reads this line.
    match error {
        true => log!("browser: agent page {tab} {request} error: {text}"),
        false => log!("browser: agent page {tab} {request} {}", text.len()),
    }
    agents_core::update(move |agents, cx| agents.page_done(request, text, error, cx));
}

/// The element an agent's action in a tab (bc_agent_page) is about to click or type in, for the
/// agent's pointer over the page.
unsafe extern "C" fn agent_page_element(_: *mut c_void, tab: bc::bc_id, request: u64, b: bc::bc_rect) {
    // check.sh reads this line.
    log!("browser: agent element {tab} {request} at {} {} {} {}", b.x, b.y, b.width, b.height);
    let element = Bounds::new(
        point(px(b.x as f32), px(b.y as f32)),
        size(px(b.width as f32), px(b.height as f32)),
    );
    agents_core::update(move |agents, cx| {
        let Some(tag) = agents.ref_of(request) else { return };
        update_shells_in(cx, |shell, cx| shell.agent_pointing(tab, tag.clone().into(), element, cx));
    });
}

/// The text of a tab's page that bc_tab_read_text asked for, or null if it had none.
unsafe extern "C" fn tab_text(_: *mut c_void, tab: bc::bc_id, request: u64, text: *const c_char) {
    let text = (!text.is_null())
        .then(|| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned());
    // check.sh reads this line.
    log!("browser: tab text {tab} {request} {}", text.as_ref().map_or(0, String::len));
    agents_core::update(move |agents, cx| agents.text_read(request, text, cx));
}

/// The page menu's Open Link in New Window and Open Link in Incognito Window: `url` opens in a
/// window from `window` as ⌘N's, or with `private` in a private one in `profile`'s session as
/// ⇧⌘N's, and shows there.
unsafe extern "C" fn window_open_link(
    _: *mut c_void,
    window: bc::bc_id,
    url: *const c_char,
    profile: bc::bc_id,
    private: bool,
) {
    let url = unsafe { CStr::from_ptr(url) }.to_string_lossy().into_owned();
    update_app(|cx| {
        // A regular window takes its Spaces from a regular one, as ⌘N's does from a private one.
        let from = WINDOWS.with_borrow(|windows| {
            let this = windows.iter().find(|w| w.id == window && (private || !w.private));
            this.or_else(|| windows.iter().find(|w| !w.private)).cloned()
        });
        let private = private.then_some(profile);
        if let Some(opened) = from.and_then(|from| open_window(Some(from), private, cx)) {
            open_tab(opened, &url);
        }
    });
}

unsafe extern "C" fn launching(_: *mut c_void) {
    unsafe { platform::host_watch_finished_launching(finished_launching) };
    let Ok(profile) = CString::new(profile_dir()) else {
        return;
    };
    // Where the front window was, which the browser window opens at again.
    let saved = std::fs::read_to_string(launch_frame_path()).ok().and_then(|text| {
        let numbers: Vec<f64> = text.split_whitespace().filter_map(|n| n.parse().ok()).collect();
        let [x, y, width, height] = numbers[..] else {
            return None;
        };
        placed(bc::bc_rect { x, y, width, height })
    });
    let bc::bc_rect { x, y, width, height } = saved.unwrap_or_else(|| {
        let (width, height) = WINDOW_SIZE;
        let (mut x, mut y) = (0., 0.);
        unsafe { platform::host_centered(width, height, &mut x, &mut y) };
        bc::bc_rect { x, y, width, height }
    });
    unsafe { platform::host_show_launch_window(profile.as_ptr(), x, y, width, height) };
    // check.sh reads this line.
    log!("browser: launch window frame {x:.0} {y:.0} {width:.0} {height:.0}");
    // The launch's times, from the process's start, for the performance gate.
    log!("browser: launch window at {:.1} ms", unsafe { platform::host_since_launch() });
    unsafe { platform::host_dock_dot_launch(profile.as_ptr()) };
}

/// Back from another app, such as System Settings, where the user may have changed the default
/// browser or given the app Full Disk Access.
extern "C" fn became_active() {
    // Later: AppKit can say so from inside the update that activated the app to show a window.
    update_app_later(|cx| {
        settings::check_default_browser(cx);
        import_host::became_active(cx);
        // GTK's settings file, Linux's last word, says nothing as it changes.
        #[cfg(target_os = "linux")]
        follow_reduce_motion(cx);
    });
}

/// Reads the system's Reduce Motion as the first window opens, before its first frame, and again
/// as the system says it changed.
fn follow_reduce_motion(cx: &mut App) {
    #[cfg(not(target_os = "linux"))]
    set_reduce_motion(unsafe { platform::host_reduce_motion() }, cx);
    #[cfg(target_os = "linux")]
    reduce_motion::read(cx);
}

#[cfg(not(target_os = "linux"))]
extern "C" fn reduce_motion_changed() {
    update_app_later(follow_reduce_motion);
}

/// What GPUI's animations and the UI's own motion follow (`App::reduce_motion`).
fn set_reduce_motion(reduce: bool, cx: &mut App) {
    if reduce != cx.reduce_motion() {
        // check.sh reads this line.
        log!("browser: reduce motion {}", if reduce { "on" } else { "off" });
        cx.set_reduce_motion(reduce);
    }
}

unsafe extern "C" fn stopping(_: *mut c_void) {
    log!("browser: stopping");
    // The recovery words a Copy put on the clipboard don't outlast the app (settings.rs).
    unsafe { platform::host_forget_secret() };
}

// The next launch's first window opens where the regular window in front is, showing its sidebar
// until its UI has drawn. Taken as the app stops being the active one and on the user's Quit, as a
// window is already hidden when it closes, and the frame alone as the last regular window closes.
// Nothing of a private window is kept.
extern "C" fn launch_picture_on_resigning() {
    // Later: AppKit can say so from inside an update of ours, such as one that hides the app.
    update_app_later(save_launch_picture);
}

fn save_launch_picture(cx: &mut App) {
    // The regular window in front, which the next launch opens first.
    let Some(window) = WINDOWS.with_borrow(|windows| windows.iter().find(|w| !w.private).cloned())
    else {
        return;
    };
    save_launch_frame(window.ui_view);
    // A launch that doesn't bring the windows back shows their Spaces without Today tabs, so a
    // picture of this sidebar would show tabs that aren't there. Without one it shows the plain
    // window, as a new profile's first launch does.
    if !unsafe { bc::bc_restore_windows() } {
        forget_launch_pictures();
        // check.sh reads this line.
        log!("browser: launch picture not taken");
        return;
    }
    let width = window.shell.upgrade().map_or(0., |shell| shell.read(cx).card_left());
    let Ok(profile) = CString::new(profile_dir()) else {
        return;
    };
    let profile = profile.as_ptr();
    let taken = unsafe { platform::host_save_launch_picture(window.ui_view, profile, width.into()) };
    // check.sh reads this line.
    log!("browser: launch picture {}", if taken { "taken" } else { "not taken" });
}

/// Deletes the launch pictures the host kept in the profile directory, one a window size
/// (LaunchPicturePath in platform/mac/host.mm).
fn forget_launch_pictures() {
    let Ok(files) = std::fs::read_dir(profile_dir()) else {
        return;
    };
    for file in files.flatten() {
        let name = file.file_name();
        if name.to_string_lossy().starts_with("launch-picture-") {
            _ = std::fs::remove_file(file.path());
        }
    }
}

/// Keeps the frame of the browser window of `ui_view` for the next launch's first window to open
/// at. Out of full screen only: the core keeps the frame a window in full screen comes back at.
fn save_launch_frame(ui_view: *mut c_void) {
    let (mut x, mut y, mut width, mut height) = (0., 0., 0., 0.);
    if unsafe { platform::host_window_frame(ui_view, &mut x, &mut y, &mut width, &mut height) } {
        _ = std::fs::write(launch_frame_path(), format!("{x} {y} {width} {height}\n"));
    }
}

/// A Space's or a folder's name, or a Space's icon, which `copy` copies as bc_space_name does.
fn name(
    copy: unsafe extern "C" fn(bc::bc_id, *mut c_char, usize) -> usize,
    id: u64,
) -> SharedString {
    let length = unsafe { copy(id, std::ptr::null_mut(), 0) };
    let mut name = vec![0u8; length + 1];
    unsafe { copy(id, name.as_mut_ptr().cast(), name.len()) };
    name.truncate(length);
    String::from_utf8_lossy(&name).into_owned().into()
}

/// The ids the core's `copy` copies.
fn ids(copy: unsafe extern "C" fn(*mut bc::bc_id, usize) -> usize) -> Vec<bc::bc_id> {
    let count = unsafe { copy(std::ptr::null_mut(), 0) };
    let mut ids = vec![0; count];
    let count = unsafe { copy(ids.as_mut_ptr(), ids.len()) };
    ids.truncate(count);
    ids
}

unsafe extern "C" fn spaces_changed(_: *mut c_void) {
    let profiles: Vec<_> = ids(bc::bc_profiles)
        .into_iter()
        .map(|profile| (profile, name(bc::bc_profile_name, profile)))
        .collect();
    let spaces: Vec<_> = ids(bc::bc_spaces)
        .into_iter()
        .map(|space| {
            let mut color = 0;
            let has_color = unsafe { bc::bc_space_color(space, &mut color) };
            let profile = unsafe { bc::bc_space_profile(space) };
            let name_of = name(bc::bc_space_name, space);
            let icon = name(bc::bc_space_icon, space);
            let agent = name(bc::bc_space_agent_name, space);
            let theme = name(bc::bc_space_theme, space);
            (space, name_of, has_color.then_some(color), profile, icon, agent, theme)
        })
        .collect();
    update_shells(|shell, cx| {
        shell.set_profiles(profiles.clone(), cx);
        shell.set_spaces(spaces.clone(), cx);
    });
    update_app(settings::refresh);
    // The rules name their Spaces by place, which a Space moved or deleted changes.
    link_rules_host::save();
    // After the update under way, as an import waiting here makes Spaces of its own.
    update_app_later(import_host::spaces_changed);
}

/// After a crash, with Send Crash Reports off: a toast asks whether to send its report, until the
/// user answers, rather than an alert in the way of the windows coming back.
unsafe extern "C" fn crash_report_offered(_: *mut c_void) {
    let button = |label: &str, name, send| toast::Button {
        label: toast::Label::Text(label.to_owned().into()),
        name,
        run: Rc::new(move |_, _| unsafe { bc::bc_answer_crash_report(send) }),
    };
    let buttons =
        vec![button("Send Report", "send", true), button("Don't Send", "don't send", false)];
    let text = concat!(env!("APP_NAME"), " quit unexpectedly.");
    update_app(move |cx| {
        with_overlays(move |cx| toast::show_until_answered(text, buttons, cx), cx)
    });
}

unsafe extern "C" fn archive_note_changed(_: *mut c_void) {
    // Before the toast's window is made, as at launch, making it shows the note.
    update_app_later(show_archive_note);
}

/// The note on the first tabs auto-archive took, as Arc's banner explains its own, while it's still
/// to be answered: a toast that stays until one of its buttons is pressed, as the crash report's
/// offer does, so it's never missed. Both buttons dismiss it for good.
fn show_archive_note(cx: &mut App) {
    if !unsafe { bc::bc_archive_note() } {
        return;
    }
    let view = toast::Button {
        label: toast::Label::Text("View Archive".into()),
        name: "view archive",
        run: Rc::new(|_, cx| {
            unsafe { bc::bc_dismiss_archive_note() };
            let shell = WINDOWS.with_borrow(|windows| windows.first().map(|w| w.shell.clone()));
            if let Some(shell) = shell {
                let view = |shell: &mut Shell, cx: &mut Context<Shell>| {
                    shell.run(Command::ViewArchive, "archive note", cx)
                };
                shell.update(cx, view).ok();
            }
        }),
    };
    let dismiss = toast::Button {
        label: toast::Label::Glyph("icons/x.svg"),
        name: "dismiss",
        run: Rc::new(|_, _| unsafe { bc::bc_dismiss_archive_note() }),
    };
    let text = format!("Tabs unseen for {} went to the archive.", settings::archive_after_words());
    // Launch shows it once the toast's window is made, and the core's change can ask again.
    if toast::showing(&text, cx) {
        return;
    }
    toast::show_until_answered(&text, vec![view, dismiss], cx);
}

unsafe extern "C" fn history_added(_: *mut c_void, profile: bc::bc_id, added: usize) {
    log!("browser: history added {added} to profile {profile}");
    update_app_later(move |cx| import_host::history_added(added, cx));
}

unsafe extern "C" fn addresses_added(_: *mut c_void, profile: bc::bc_id, added: usize) {
    // check.sh waits for this line.
    log!("browser: addresses added {added} to profile {profile}");
    update_app_later(move |cx| import_host::addresses_added(added, cx));
}

unsafe extern "C" fn favorites_changed(_: *mut c_void) {
    // Each with its holder: the Space it belongs to under Each Space, else its profile.
    let holder = |tab| unsafe {
        match bc::bc_favorite_space(tab) {
            0 => bc::bc_tab_profile(tab),
            space => space,
        }
    };
    let tabs: Vec<_> = ids(bc::bc_favorites).into_iter().map(|tab| (tab, holder(tab))).collect();
    update_shells(|shell, cx| shell.set_favorites(tabs.clone(), cx));
    update_app(settings::refresh);
}

unsafe extern "C" fn space_changed(_: *mut c_void, space: bc::bc_id) {
    let list = |copy: unsafe extern "C" fn(bc::bc_id, *mut bc::bc_id, usize) -> usize| {
        let count = unsafe { copy(space, std::ptr::null_mut(), 0) };
        let mut tabs = vec![0; count];
        let count = unsafe { copy(space, tabs.as_mut_ptr(), tabs.len()) };
        tabs.truncate(count);
        tabs
    };
    let (pinned, today) = (list(bc::bc_space_pinned), list(bc::bc_space_today));
    let mut splits = Vec::new();
    for &tab in pinned.iter().chain(&today) {
        // The core's splits have up to three tabs.
        let mut split = [0; 3];
        let count = unsafe { bc::bc_tab_split(tab, split.as_mut_ptr(), split.len()) };
        if count > 1 && split[0] == tab {
            let tabs = split[..count.min(split.len())].to_vec();
            let shares: Vec<f64> =
                tabs.iter().map(|&tab| unsafe { bc::bc_tab_split_share(tab) }).collect();
            // Even shares are the core's integer split, which the shell draws the same way.
            let even = shares.iter().all(|&share| share == shares[0]);
            splits.push(Split {
                tabs,
                stacked: unsafe { bc::bc_tab_split_stacked(tab) },
                shares: if even { Vec::new() } else { shares },
                expanded: unsafe { bc::bc_tab_split_expanded(tab) },
            });
        }
    }
    let pinned_shut = unsafe { !bc::bc_space_pinned_open(space) };
    let mut folders = Folders { pinned_shut, ..Default::default() };
    for &row in &pinned {
        let mut open = false;
        if unsafe { bc::bc_folder(row, &mut open) } {
            folders.folders.insert(row, (name(bc::bc_folder_name, row), open));
            let icon = name(bc::bc_folder_icon, row);
            if !icon.is_empty() {
                folders.icons.insert(row, icon);
            }
        }
        match unsafe { bc::bc_folder_of(row) } {
            0 => {}
            folder => _ = folders.parents.insert(row, folder),
        }
    }
    update_shells(|shell, cx| {
        let (pinned, today) = (pinned.clone(), today.clone());
        shell.set_tabs(space, pinned, today, splits.clone(), folders.clone(), cx)
    });
}

/// A private window's name, in its sidebar and the Window menu: Chrome's, and its profile's while
/// there's more than one, as each has its own private session.
fn private_name(profile: bc::bc_id) -> SharedString {
    match ids(bc::bc_profiles).len() {
        0 | 1 => "Incognito".into(),
        _ => format!("Incognito · {}", name(bc::bc_profile_name, profile)).into(),
    }
}

unsafe extern "C" fn window_space_shown(_: *mut c_void, window: bc::bc_id, space: bc::bc_id) {
    // check.sh reads this line, as a relaunch brings back each window's Space.
    log!("browser: window {window} in space {}", name(bc::bc_space_name, space));
    if name(bc::bc_space_agent, space).is_empty() {
        WINDOWS.with_borrow_mut(|windows| {
            if let Some(window) = windows.iter_mut().find(|w| w.id == window) {
                window.users_space = space;
            }
        });
    }
    update_window(window, |shell, cx| shell.set_space(space, cx));
}

unsafe extern "C" fn window_selected(_: *mut c_void, window: bc::bc_id, tab: bc::bc_id) {
    // check.sh reads this line, as a tab's page moves between windows.
    log!("browser: window {window} shows {tab}");
    browser::log_handover("window shows the tab", &format!("window {window}, tab {tab}"));
    update_window(window, |shell, cx| {
        shell.set_selected(tab, cx);
        find_bar::tab_shown(window, tab, cx);
        zoom_hud::tab_shown(tab, cx);
        offer::tab_shown(window, tab, cx);
    });
}

unsafe extern "C" fn window_page_elsewhere(_: *mut c_void, window: bc::bc_id, tab: bc::bc_id) {
    // check.sh reads this line.
    log!("browser: window {window} page elsewhere {tab}");
    let (stage, detail) = match tab {
        0 => ("taker told", format!("window {window}")),
        _ => ("giver told", format!("window {window}, tab {tab}")),
    };
    browser::log_handover(stage, &detail);
    update_window(window, |shell, cx| shell.page_elsewhere(tab, cx));
    update_app(|cx| offer::page_elsewhere(window, tab != 0, cx));
}

// check.sh reads these lines. Test pages report what reached them through their title.
unsafe extern "C" fn tab_loaded(_: *mut c_void, tab: bc::bc_id, url: *const c_char) {
    let url = unsafe { CStr::from_ptr(url) }.to_string_lossy();
    log!("browser: loaded {tab} {url}");
}

unsafe extern "C" fn tab_state_changed(
    _: *mut c_void,
    tab: bc::bc_id,
    state: *const bc::bc_tab_state,
) {
    let state = unsafe { &*state };
    let page = Page {
        url: unsafe { CStr::from_ptr(state.url) }
            .to_string_lossy()
            .into_owned()
            .into(),
        can_go_back: state.can_go_back,
        can_go_forward: state.can_go_forward,
        loading: state.loading,
        unloaded: state.unloaded,
        away_from_pinned_url: state.away_from_pinned_url,
        audible: state.audible,
        muted: state.muted,
        icon: unsafe { CStr::from_ptr(state.icon) }.to_string_lossy().into_owned().into(),
        trouble: match state.trouble as u32 {
            bc::BC_PAGE_CRASHED => Trouble::Crashed,
            bc::BC_PAGE_HUNG => Trouble::Hung,
            bc::BC_PAGE_NETWORK_ERROR => Trouble::NetworkError(state.net_error),
            _ => Trouble::Fine,
        },
        system_engine: state.system_engine,
        camera: state.camera,
        microphone: state.microphone,
        sharing: state.sharing,
        shared: state.shared,
        share_instead: state.share_instead,
        media: state.media,
        media_playing: state.media_playing,
        media_title: unsafe { CStr::from_ptr(state.media_title) }
            .to_string_lossy()
            .into_owned()
            .into(),
        media_artist: unsafe { CStr::from_ptr(state.media_artist) }
            .to_string_lossy()
            .into_owned()
            .into(),
        media_next: state.media_next,
        media_picture_in_picture: state.media_picture_in_picture,
        developer_mode: state.developer_mode,
    };
    if unsafe { bc::bc_tab_window(tab) } == 0 {
        update_app(|cx| link_preview::set_url(tab, page.url.clone(), cx));
    }
    update_shells(|shell, cx| shell.set_page(tab, page.clone(), cx));
}

unsafe extern "C" fn tab_title_changed(_: *mut c_void, tab: bc::bc_id, title: *const c_char) {
    let title = unsafe { CStr::from_ptr(title) }.to_string_lossy().into_owned();
    log!("browser: title {tab} {title}");
    update_shells(|shell, cx| shell.set_title(tab, title.clone().into(), cx));
}

unsafe extern "C" fn tab_favicon_changed(
    _: *mut c_void,
    tab: bc::bc_id,
    png: *const u8,
    size: usize,
) {
    let favicon = (!png.is_null()).then(|| {
        let png = unsafe { std::slice::from_raw_parts(png, size) }.to_vec();
        Arc::new(Image::from_bytes(ImageFormat::Png, png))
    });
    log!("browser: favicon {tab} {size} bytes");
    update_shells(|shell, cx| shell.set_favicon(tab, favicon.clone(), cx));
}

unsafe extern "C" fn tab_history(
    _: *mut c_void,
    tab: bc::bc_id,
    forward: bool,
    entries: *const bc::bc_history_entry,
    count: usize,
) {
    let entries = match count {
        0 => &[][..],
        _ => unsafe { std::slice::from_raw_parts(entries, count) },
    };
    let text = |text: *const c_char| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();
    let pages: Vec<_> = (entries.iter())
        .map(|entry| shell::HistoryPage {
            offset: entry.offset,
            url: text(entry.url).into(),
            title: text(entry.title).into(),
            favicon: (!entry.favicon.is_null()).then(|| {
                let png = unsafe { std::slice::from_raw_parts(entry.favicon, entry.favicon_size) };
                Arc::new(Image::from_bytes(ImageFormat::Png, png.to_vec()))
            }),
        })
        .collect();
    let way = if forward { "forward" } else { "back" };
    log!("browser: history {tab} {way} {}", pages.len());
    update_shells(|shell, cx| shell.show_history(tab, forward, pages.clone(), cx));
}

unsafe extern "C" fn archive_changed(
    _: *mut c_void,
    tabs: *const bc::bc_archived_tab,
    count: usize,
) {
    let tabs = match tabs.is_null() {
        true => &[][..],
        false => unsafe { std::slice::from_raw_parts(tabs, count) },
    };
    let text = |text| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned().into();
    let archive: Vec<_> = tabs
        .iter()
        .map(|tab| Archived {
            id: tab.id,
            url: text(tab.url),
            title: text(tab.title),
            archived: tab.archived,
        })
        .collect();
    // check.sh waits for this line.
    log!("browser: archive {count} tabs");
    update_shells(|shell, cx| shell.set_archive(archive.clone(), cx));
}

unsafe extern "C" fn downloads_changed(
    _: *mut c_void,
    downloads: *const bc::bc_download,
    count: usize,
) {
    let downloads = match downloads.is_null() {
        true => &[][..],
        false => unsafe { std::slice::from_raw_parts(downloads, count) },
    };
    let text = |text| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned().into();
    let downloads: Vec<_> = downloads
        .iter()
        .map(|download| Download {
            id: download.id,
            name: text(download.name),
            url: text(download.url),
            path: text(download.path),
            received: download.received,
            total: download.total,
            state: match download.state as u32 {
                bc::BC_DOWNLOAD_RUNNING => DownloadState::Running,
                bc::BC_DOWNLOAD_PAUSED => DownloadState::Paused,
                bc::BC_DOWNLOAD_DONE => DownloadState::Done,
                bc::BC_DOWNLOAD_CANCELLED => DownloadState::Cancelled,
                _ => DownloadState::Failed,
            },
            open_when_done: download.open_when_done,
            held: download.held,
            started: download.started,
        })
        .collect();
    update_shells(|shell, cx| shell.set_downloads(downloads.clone(), cx));
}

unsafe extern "C" fn download_icon(
    _: *mut c_void,
    download: bc::bc_id,
    png: *const u8,
    size: usize,
) {
    let icon = (!png.is_null()).then(|| {
        let png = unsafe { std::slice::from_raw_parts(png, size) }.to_vec();
        Arc::new(Image::from_bytes(ImageFormat::Png, png))
    });
    update_shells(|shell, cx| shell.set_download_icon(download, icon.clone(), cx));
}

unsafe extern "C" fn extensions_changed(
    _: *mut c_void,
    profile: bc::bc_id,
    extensions: *const bc::bc_extension,
    count: usize,
) {
    let extensions = match extensions.is_null() {
        true => &[][..],
        false => unsafe { std::slice::from_raw_parts(extensions, count) },
    };
    let text = |text| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned().into();
    let extensions: Vec<_> = extensions
        .iter()
        .map(|extension| Extension {
            id: text(extension.id),
            name: text(extension.name),
            enabled: extension.enabled,
            pinned: extension.pinned,
            options: text(extension.options),
            removable: extension.removable,
            incognito: extension.incognito,
            crashed: extension.crashed,
            commands: match extension.commands.is_null() {
                true => &[][..],
                false => unsafe {
                    std::slice::from_raw_parts(extension.commands, extension.command_count)
                },
            }
            .iter()
            .map(|command| ExtensionCommand {
                name: text(command.name),
                description: text(command.description),
                keystroke: keystroke(&bc::bc_key {
                    key: command.key,
                    modifiers: command.modifiers,
                    editing: false,
                    latin_key: std::ptr::null(),
                })
                .into(),
                suggested: command.suggested,
            })
            .collect(),
        })
        .collect();
    update_shells(|shell, cx| shell.set_extensions(profile, extensions.clone(), cx));
    // Over the window in front, once.
    update_window(front().0, |shell, cx| shell.tell_crashed_extension(profile, cx));
}

unsafe extern "C" fn window_extension_actions(
    _: *mut c_void,
    window: bc::bc_id,
    actions: *const bc::bc_extension_action,
    count: usize,
) {
    let actions = match actions.is_null() {
        true => &[][..],
        false => unsafe { std::slice::from_raw_parts(actions, count) },
    };
    let text = |text| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned().into();
    let actions = actions
        .iter()
        .map(|action| ExtensionAction {
            id: text(action.id),
            title: text(action.title),
            icon: (!action.icon.is_null()).then(|| {
                let png = unsafe { std::slice::from_raw_parts(action.icon, action.icon_size) };
                Arc::new(Image::from_bytes(ImageFormat::Png, png.to_vec()))
            }),
            badge: text(action.badge),
            badge_color: action.badge_color,
            enabled: action.enabled,
            pinned: action.pinned,
        })
        .collect();
    update_window(window, |shell, cx| shell.set_extension_actions(actions, cx));
}

unsafe extern "C" fn window_extension_question(
    _: *mut c_void,
    window: bc::bc_id,
    question: *const bc::bc_extension_question,
) {
    let Some(question) = (unsafe { question.as_ref() }) else {
        if window == front().0 {
            // check.sh waits for this line.
            log!("browser: extension question none");
            update_app(dialog::withdraw_chrome);
        }
        return;
    };
    let id = question.id;
    let text = |text| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();
    let lines = |texts: *const *const c_char| match texts.is_null() {
        true => Vec::new(),
        false => unsafe { std::slice::from_raw_parts(texts, question.warning_count) }
            .iter()
            .map(|&line| text(line))
            .collect(),
    };
    let warnings = lines(question.warnings);
    // check.sh waits for this line.
    log!("browser: extension question {} asks {}", text(question.extension), warnings.join("; "));
    let list: Vec<_> = warnings
        .into_iter()
        .zip(lines(question.details))
        .map(|(warning, details)| {
            (warning.into(), details.lines().map(|line| line.to_owned().into()).collect())
        })
        .collect();
    let question = dialog::Question {
        icon: (!question.icon.is_null()).then(|| {
            let png = unsafe { std::slice::from_raw_parts(question.icon, question.icon_size) };
            Arc::new(Image::from_bytes(ImageFormat::Png, png.to_vec()))
        }),
        message: text(question.title).into(),
        // Chrome's heading, such as "It can:", leads its warnings, and there's none without them.
        detail: if list.is_empty() { "".into() } else { text(question.heading).into() },
        list,
        action: text(question.accept).into(),
        cancel: text(question.cancel).into(),
        tone: dialog::Tone::Accent,
        also: None,
        fields: Vec::new(),
        check: None,
        on_return: dialog::Return::Action,
    };
    let mut asked = false;
    if window == front().0 {
        update_app(|cx| {
            asked = true;
            let answered = Box::new(move |accept, _: &mut App| unsafe {
                bc::bc_extension_answer(id, accept)
            });
            dialog::ask_chrome(question, answered, cx);
        });
    }
    // With no UI to ask in, Chrome isn't left waiting.
    if !asked {
        unsafe { bc::bc_extension_answer(id, false) };
    }
}

unsafe extern "C" fn window_side_panel(
    _: *mut c_void,
    window: bc::bc_id,
    panel: *const bc::bc_side_panel,
) {
    let text = |text| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned().into();
    let panel = unsafe { panel.as_ref() }.map(|panel| SidePanel {
        extension: text(panel.extension),
        name: text(panel.name),
        icon: (!panel.icon.is_null()).then(|| {
            let png = unsafe { std::slice::from_raw_parts(panel.icon, panel.icon_size) };
            Arc::new(Image::from_bytes(ImageFormat::Png, png.to_vec()))
        }),
        title: text(panel.title),
        width: panel.width as f32,
    });
    update_window(window, |shell, cx| shell.set_side_panel(panel, cx));
}

unsafe extern "C" fn window_extension_popup(
    _: *mut c_void,
    window: bc::bc_id,
    extension: *const c_char,
) {
    let extension = unsafe { CStr::from_ptr(extension) }.to_string_lossy();
    if WINDOWS.with_borrow(|windows| windows.iter().any(|open| open.id == window)) {
        // check.sh waits for these lines.
        match extension.is_empty() {
            true => log!("browser: extension popup none"),
            false => log!("browser: extension popup {extension}"),
        }
    }
}

/// On Linux and Windows, Chrome asks whether to quit, or close the last private window, cancelling
/// `count` downloads running. In the words of Chrome's alert on the Mac, with the app's name where
/// it names Chromium, and ↩ on Continue Downloading, as there.
unsafe extern "C" fn window_downloads_question(
    _: *mut c_void,
    window: bc::bc_id,
    count: c_int,
    last_private: bool,
) {
    let question = dialog::Question {
        icon: None,
        message: match count {
            1 => "Download is in progress",
            _ => "Downloads are in progress",
        }
        .into(),
        detail: match last_private {
            true => "Quit Incognito mode anyway?",
            false => concat!("Quit ", env!("APP_NAME"), " anyway?"),
        }
        .into(),
        list: Vec::new(),
        action: "Quit".into(),
        cancel: "Continue Downloading".into(),
        // Quit cancels the downloads, which can't be taken back.
        tone: dialog::Tone::Destructive,
        also: None,
        fields: Vec::new(),
        check: None,
        on_return: dialog::Return::Cancel,
    };
    let answered =
        Box::new(move |cancel, _: &mut App| unsafe { bc::bc_window_answer_downloads(window, cancel) });
    // With no UI to ask in, the browser stays open, as it does with no dialog to ask in.
    if CX.with_borrow(Option::is_none) {
        unsafe { bc::bc_window_answer_downloads(window, false) };
        return;
    }
    // A private window's close can ask from inside the app's own call.
    update_app_later(move |cx| dialog::ask_chrome(question, answered, cx));
}

unsafe extern "C" fn window_show_downloads(_: *mut c_void, window: bc::bc_id) {
    update_window(window, |shell, cx| shell.show_downloads(cx));
}

unsafe extern "C" fn page_favicon(_: *mut c_void, url: *const c_char, png: *const u8, size: usize) {
    let url = unsafe { CStr::from_ptr(url) }.to_string_lossy().into_owned();
    let favicon = (!png.is_null()).then(|| {
        let png = unsafe { std::slice::from_raw_parts(png, size) }.to_vec();
        Arc::new(Image::from_bytes(ImageFormat::Png, png))
    });
    update_app(|cx| command_bar::page_favicon(&url, favicon.clone(), cx));
    update_shells(|shell, cx| shell.set_page_favicon(url.clone().into(), favicon.clone(), cx));
}

unsafe extern "C" fn tab_opened(
    _: *mut c_void,
    window: bc::bc_id,
    tab: bc::bc_id,
    opener: bc::bc_id,
    foreground: bool,
) {
    let side = if foreground { "foreground" } else { "background" };
    log!("browser: tab {tab} opened by {opener} in the {side}");
    // A link ⌥⌘-clicked, or opened in a split view from the page's menu, joined its page's split,
    // which ⌘Z takes back as it does the user's other ways in.
    let mut split = [0; 3];
    let count = unsafe { bc::bc_tab_split(tab, split.as_mut_ptr(), split.len()) };
    if opener != 0 && count > 1 && split[..count.min(3)].contains(&opener) {
        update_app_later(move |cx| split_opened(window, tab, cx));
    }
}

// Called while Chromium closes the tab, which can be inside a GPUI update that asked for it, so
// the shell hears of it from space_changed instead, and Site Controls once that update is over.
unsafe extern "C" fn tab_closed(_: *mut c_void, tab: bc::bc_id) {
    log!("browser: tab {tab} closed");
    find_bar::forget(tab);
    offer::forget(tab);
    if let Some(cx) = CX.with_borrow(Clone::clone) {
        let executor = cx.foreground_executor().clone();
        executor
            .spawn(async move {
                cx.update(|cx| {
                    site_controls::tab_closed(tab, cx);
                    capture::tab_closed(tab, cx);
                    link_preview::tab_closed(tab, cx);
                })
            })
            .detach();
    }
}

/// Shows `tab`'s page in Safari's engine, in the tab, and has its profile keep the site for it: the
/// protected-video note's button and Site Controls' On.
fn use_system_engine(tab: u64) {
    // check.sh reads this line.
    let used = unsafe { bc::bc_tab_set_system_engine(tab, true) };
    log!("browser: system engine {tab} {}", if used { "on" } else { "refused" });
}

unsafe extern "C" fn tab_site_controls(
    _: *mut c_void,
    tab: bc::bc_id,
    controls: *const bc::bc_site_controls,
) {
    let controls = unsafe { &*controls };
    let string = |text: *const c_char| -> SharedString {
        unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned().into()
    };
    let value = |value: u32| match value {
        bc::BC_SITE_ALLOW => site_controls::Value::Allow,
        bc::BC_SITE_BLOCK => site_controls::Value::Block,
        _ => site_controls::Value::Ask,
    };
    let bit = |each: site_controls::Value| match each {
        site_controls::Value::Allow => bc::BC_SITE_ALLOW,
        site_controls::Value::Block => bc::BC_SITE_BLOCK,
        site_controls::Value::Ask => bc::BC_SITE_ASK,
    };
    let permissions = match controls.count {
        0 => &[][..],
        count => unsafe { std::slice::from_raw_parts(controls.permissions, count) },
    };
    let site = site_controls::Site {
        site: string(controls.site),
        connection: match controls.connection as u32 {
            bc::BC_CONNECTION_SECURE => site_controls::Connection::Secure,
            bc::BC_CONNECTION_INTERNAL => site_controls::Connection::Internal,
            _ => site_controls::Connection::NotSecure,
        },
        connection_summary: string(controls.connection_summary),
        certificate: controls.certificate,
        permissions: permissions
            .iter()
            .map(|permission| site_controls::Permission {
                kind: permission.type_,
                key: string(permission.key),
                name: string(permission.name),
                value: value(permission.value as u32),
                state: string(permission.state),
                choices: (site_controls::Value::ALL.into_iter())
                    .filter(|&each| permission.choices & (1 << bit(each)) != 0)
                    .collect(),
            })
            .collect(),
        reset: Some(string(controls.reset)).filter(|reset| !reset.is_empty()),
        system_engine: match (controls.system_engine, controls.system_engine_offered) {
            (true, _) => Some(true),
            (false, true) => Some(false),
            (false, false) => None,
        },
        ad_blocking: match controls.ad_blocking as u32 {
            bc::BC_AD_BLOCKING_ON => Some(true),
            bc::BC_AD_BLOCKING_OFF => Some(false),
            _ => None,
        },
        developer_mode: match controls.developer_mode as u32 {
            bc::BC_DEVELOPER_MODE_ON => Some(true),
            bc::BC_DEVELOPER_MODE_OFF => Some(false),
            _ => None,
        },
        logins: logins(controls.logins, controls.login_count),
    };
    update_app(|cx| site_controls::changed(tab, site, cx));
}

/// The core's `count` logins at `logins`, as the app keeps them.
fn logins(logins: *const bc::bc_login, count: usize) -> Vec<passwords::Login> {
    let string = |text: *const c_char| -> SharedString {
        unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned().into()
    };
    let logins = match count {
        0 => &[][..],
        count => unsafe { std::slice::from_raw_parts(logins, count) },
    };
    (logins.iter())
        .map(|login| passwords::Login {
            site: string(login.site),
            username: string(login.username),
            never: login.never,
            key: string(login.key),
        })
        .collect()
}

/// The profile's saved addresses and cards, which Settings asked the core for.
unsafe extern "C" fn profile_autofill_changed(
    _: *mut c_void,
    profile: bc::bc_id,
    entries: *const bc::bc_autofill_entry,
    count: usize,
) {
    let string = |text: *const c_char| -> SharedString {
        unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned().into()
    };
    let entries = match count {
        0 => &[][..],
        count => unsafe { std::slice::from_raw_parts(entries, count) },
    };
    let saved: Vec<settings::Saved> = (entries.iter())
        .map(|entry| settings::Saved {
            title: string(entry.title),
            detail: string(entry.detail),
            card: entry.card,
            key: string(entry.key),
        })
        .collect();
    update_app(|cx| settings::autofill_changed(profile, saved, cx));
}

/// The profile's logins, which Settings asked the core for.
unsafe extern "C" fn profile_logins_changed(
    _: *mut c_void,
    profile: bc::bc_id,
    list: *const bc::bc_login,
    count: usize,
) {
    let list = logins(list, count);
    update_app(|cx| settings::logins_changed(profile, list, cx));
}

/// A string the core passed, for as long as the app keeps it.
fn core_text(text: *const c_char) -> SharedString {
    match text.is_null() {
        true => SharedString::default(),
        false => unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned().into(),
    }
}

/// The browser's account, for Settings. Never logged here: the email is the user's.
unsafe extern "C" fn sync_changed(_: *mut c_void, state: *const bc::bc_sync_state) {
    let Some(state) = (unsafe { state.as_ref() }) else {
        return;
    };
    let status = match state.status as u32 {
        bc::BC_SYNC_SIGNING_IN => account::Status::SigningIn,
        bc::BC_SYNC_FIRST_DEVICE => account::Status::FirstDevice,
        bc::BC_SYNC_NEEDS_SECRET => account::Status::NeedsSecret,
        bc::BC_SYNC_PAIRING => account::Status::Pairing,
        bc::BC_SYNC_ON => account::Status::On,
        _ => account::Status::SignedOut,
    };
    let problem = match state.problem as u32 {
        bc::BC_SYNC_PROBLEM_WRONG_WORDS => Some(account::Problem::WrongWords),
        bc::BC_SYNC_PROBLEM_DECLINED => Some(account::Problem::Declined),
        bc::BC_SYNC_PROBLEM_EXPIRED => Some(account::Problem::Expired),
        bc::BC_SYNC_PROBLEM_REMOVED => Some(account::Problem::Removed),
        bc::BC_SYNC_PROBLEM_SERVER => Some(account::Problem::Server),
        _ => None,
    };
    let state = account::State {
        status,
        problem,
        server: core_text(state.server),
        default_server: state.default_server,
        email: core_text(state.email),
        code: core_text(state.code),
    };
    if CX.with_borrow(Option::is_none) {
        SYNC_STATE.set(Some(state));
        return;
    }
    update_app_later(move |cx| settings::sync_changed(state, cx));
}

/// Another device asks to be added to the account: the dialog asks over the front window, or
/// leaves once the request has ended unanswered.
unsafe extern "C" fn sync_pairing_asked(_: *mut c_void, pairing: *const bc::bc_sync_pairing) {
    let Some(pairing) = (unsafe { pairing.as_ref() }) else {
        // check.sh waits for this line.
        log!("browser: pairing withdrawn");
        update_app_later(dialog::withdraw_account);
        return;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0., |since| since.as_secs_f64() * 1e3);
    let mut day_starts = [0.; 4];
    unsafe { platform::host_day_starts(day_starts.as_mut_ptr()) };
    let asked_at = if pairing.asked_at > 0. { pairing.asked_at } else { now };
    let signed_in = match pairing.signed_in_at > 0. {
        true => account::since(now - pairing.signed_in_at),
        false => String::new(),
    };
    let pairing = account::Pairing {
        id: core_text(pairing.id),
        name: core_text(pairing.name),
        os: core_text(pairing.os),
        code: core_text(pairing.code),
        asked: account::clock(asked_at, day_starts[0]).into(),
        signed_in: signed_in.into(),
    };
    // check.sh waits for this line.
    log!("browser: pairing asked");
    update_app_later(move |cx| {
        let id = CString::new(pairing.id.as_str()).unwrap_or_default();
        let answered = Box::new(move |add, _: &mut App| unsafe {
            bc::bc_sync_answer_pairing(id.as_ptr(), add)
        });
        dialog::ask_account(account::pairing_question(&pairing), answered, cx);
    });
}

/// The account's devices, which Settings asked the core for.
unsafe extern "C" fn sync_devices_changed(
    _: *mut c_void,
    devices: *const bc::bc_sync_device,
    count: usize,
) {
    let devices = match count {
        0 => &[][..],
        count => unsafe { std::slice::from_raw_parts(devices, count) },
    };
    let devices = (devices.iter())
        .map(|device| account::Device {
            id: core_text(device.id),
            name: core_text(device.name),
            os: core_text(device.os),
            current: device.current,
        })
        .collect();
    update_app_later(move |cx| settings::devices_changed(devices, cx));
}

/// What came of a passwords file the import gave the core. Never logged: the counts are the
/// user's.
unsafe extern "C" fn passwords_imported(
    _: *mut c_void,
    profile: bc::bc_id,
    result: i32,
    imported: usize,
    conflicted: usize,
    skipped: usize,
) {
    let passwords = match result as u32 {
        bc::BC_PASSWORDS_IMPORTED => browser::import::Passwords::Imported {
            imported,
            conflicted,
            skipped,
        },
        bc::BC_PASSWORDS_IMPORT_UNREADABLE => browser::import::Passwords::Unreadable,
        bc::BC_PASSWORDS_IMPORT_NOT_EXPORT => browser::import::Passwords::NotExport,
        bc::BC_PASSWORDS_IMPORT_TOO_LARGE => browser::import::Passwords::TooLarge,
        _ => browser::import::Passwords::Failed,
    };
    update_app_later(move |cx| import_host::passwords_imported(profile, passwords, cx));
}

unsafe extern "C" fn tab_find_changed(
    _: *mut c_void,
    tab: bc::bc_id,
    state: *const bc::bc_find_state,
) {
    let state = unsafe { &*state };
    let find = find_bar::Find {
        open: state.open,
        text: unsafe { CStr::from_ptr(state.text) }
            .to_string_lossy()
            .into_owned(),
        matches: state.matches,
        selected: state.selected,
    };
    update_app(|cx| find_bar::changed(tab, find, cx));
}

unsafe extern "C" fn tab_offer_changed(
    _: *mut c_void,
    tab: bc::bc_id,
    offer: *const bc::bc_offer,
) {
    let offer = (!offer.is_null()).then(|| {
        let offer = unsafe { &*offer };
        let string = |text: *const c_char| -> SharedString {
            unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned().into()
        };
        let usernames = match offer.username_count {
            0 => Vec::new(),
            count => unsafe { std::slice::from_raw_parts(offer.usernames, count) }
                .iter()
                .map(|username| string(*username))
                .collect(),
        };
        let never = string(offer.never);
        let details = string(offer.details);
        offer::Offer {
            kind: match offer.kind as u32 {
                bc::BC_OFFER_KIND_PASSWORD_UPDATE => offer::Kind::Update,
                bc::BC_OFFER_KIND_ADDRESS => offer::Kind::Address,
                bc::BC_OFFER_KIND_CARD => offer::Kind::Card,
                _ => offer::Kind::Save,
            },
            title: string(offer.title),
            site: string(offer.site),
            username: string(offer.username),
            usernames,
            password_length: offer.password_length,
            accept: string(offer.accept),
            decline: string(offer.decline),
            never: (!never.is_empty()).then_some(never),
            details: details.lines().map(|line| line.to_string().into()).collect(),
        }
    });
    update_app(|cx| offer::changed(tab, offer, cx));
}

unsafe extern "C" fn tab_passwords_changed(_: *mut c_void, tab: bc::bc_id, passwords: c_int) {
    let key = match passwords as u32 {
        bc::BC_PASSWORDS_SAVED => Some(passwords::Key::Saved),
        bc::BC_PASSWORDS_OFFER => Some(passwords::Key::Offer),
        _ => None,
    };
    // check.sh reads this line.
    let words = key.map_or("none".into(), |key| format!("{key:?}").to_lowercase());
    log!("browser: key {tab} {words}");
    update_shells(|shell, cx| shell.set_key(tab, key, cx));
}

/// Tells the core the user's answer to the dialog `id` of `tab`'s page.
fn answer_page_dialog(tab: u64, id: u64, reply: shell::PageReply) {
    let text = CString::new(reply.text).unwrap_or_default();
    let password = CString::new(reply.password).unwrap_or_default();
    unsafe {
        bc::bc_tab_answer_dialog(
            tab,
            id,
            reply.accept,
            text.as_ptr(),
            password.as_ptr(),
            reply.checked,
        )
    };
}

/// A tab's page asks a dialog, such as its alert(), or no longer does (null).
unsafe extern "C" fn tab_dialog(_: *mut c_void, tab: bc::bc_id, asked: *const bc::bc_page_dialog) {
    let text = |text: *const c_char| -> SharedString {
        unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned().into()
    };
    let dialog = unsafe { asked.as_ref() }.map(|asked| shell::PageDialog {
        id: asked.id,
        kind: match asked.kind as u32 {
            bc::BC_DIALOG_CONFIRM => shell::DialogKind::Confirm,
            bc::BC_DIALOG_PROMPT => shell::DialogKind::Prompt,
            bc::BC_DIALOG_LEAVE => shell::DialogKind::Leave,
            bc::BC_DIALOG_SIGN_IN => shell::DialogKind::SignIn,
            bc::BC_DIALOG_OPEN_APP => shell::DialogKind::OpenApp,
            _ => shell::DialogKind::Alert,
        },
        title: text(asked.title),
        message: text(asked.message),
        text: text(asked.text),
        accept: text(asked.accept),
        cancel: text(asked.cancel),
        check: text(asked.check),
        username: text(asked.username),
        password: text(asked.password),
    });
    // check.sh waits for these lines.
    match &dialog {
        Some(dialog) => log!("browser: page {tab} dialog {:?} {}", dialog.kind, dialog.title),
        None => log!("browser: page {tab} dialog none"),
    }
    update_shells(|shell, cx| shell.set_page_dialog(tab, dialog.clone(), cx));
}

unsafe extern "C" fn tab_permission_asked(
    _: *mut c_void,
    tab: bc::bc_id,
    site: *const c_char,
    permissions: *const *const c_char,
    count: usize,
) {
    let string = |text: *const c_char| {
        unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
    };
    let permissions: Vec<String> = match count {
        0 => Vec::new(),
        _ => unsafe { std::slice::from_raw_parts(permissions, count) }
            .iter()
            .map(|permission| string(*permission))
            .collect(),
    };
    let question = match count {
        0 => {
            log!("browser: page {tab} asks nothing");
            None
        }
        _ => {
            let site = string(site);
            log!("browser: page {tab} asks {site} to {}", permissions.join("; "));
            Some(shell::Question {
                site: site.into(),
                permissions: permissions.into_iter().map(Into::into).collect(),
            })
        }
    };
    update_shells(|shell, cx| shell.set_question(tab, question.clone(), cx));
}

/// A tab's Cast dialog, or that it closed (a null header), for the window showing the tab.
unsafe extern "C" fn tab_cast(
    _: *mut c_void,
    tab: bc::bc_id,
    header: *const c_char,
    devices: *const bc::bc_cast_device,
    count: usize,
) {
    let string = |text: *const c_char| -> SharedString {
        unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned().into()
    };
    let cast = (!header.is_null()).then(|| {
        let devices = match count {
            0 => &[][..],
            _ => unsafe { std::slice::from_raw_parts(devices, count) },
        };
        shell::Cast {
            header: string(header),
            devices: (devices.iter())
                .map(|device| shell::CastDevice {
                    id: string(device.id),
                    name: string(device.name),
                    state: match device.state as u32 {
                        bc::BC_CAST_AVAILABLE => shell::CastState::Available,
                        bc::BC_CAST_CONNECTING => shell::CastState::Connecting,
                        bc::BC_CAST_CONNECTED => shell::CastState::Connected,
                        bc::BC_CAST_DISCONNECTING => shell::CastState::Disconnecting,
                        _ => shell::CastState::Unavailable,
                    },
                    status: string(device.status),
                })
                .collect(),
        }
    });
    // check.sh waits for these lines.
    match &cast {
        Some(cast) => {
            let devices: Vec<String> = (cast.devices.iter())
                .map(|device| format!("{} {:?}", device.name, device.state))
                .collect();
            log!("browser: cast {tab} {} devices {}", cast.header, devices.len());
            for device in devices {
                log!("browser: cast {tab} device {device}");
            }
        }
        None => log!("browser: cast {tab} closed"),
    }
    update_shells(|shell, cx| shell.set_cast(tab, cast.clone(), cx));
}

/// The user right-clicked a page: its menu, as the core has Chrome build it, in ilo's menu over the
/// window that shows it. With no window, the menu went with its page.
unsafe extern "C" fn tab_page_menu(
    _: *mut c_void,
    window: bc::bc_id,
    tab: bc::bc_id,
    x: f64,
    y: f64,
    items: *const bc::bc_page_menu_item,
    count: usize,
) {
    if window == 0 {
        log!("browser: page menu {tab} gone");
        update_app(menu::dismissed);
        return;
    }
    let items = match count {
        0 => &[][..],
        _ => unsafe { std::slice::from_raw_parts(items, count) },
    };
    let items: Vec<shell::PageMenuItem> = (items.iter())
        .map(|item| shell::PageMenuItem {
            command: item.command,
            title: unsafe { CStr::from_ptr(item.title) }.to_string_lossy().into_owned().into(),
            parent: usize::try_from(item.parent).ok(),
            enabled: item.enabled,
            checked: item.checked,
        })
        .collect();
    log!("browser: page menu {tab} at {x:.0},{y:.0}");
    let at = point(px(x as f32), px(y as f32));
    if update_window(window, |shell, cx| shell.show_page_menu(tab, items, at, cx)).is_none() {
        // No window to show it in, and the page opens no other menu until this one closes.
        unsafe { bc::bc_tab_close_page_menu(tab, 0) };
    }
}

unsafe extern "C" fn tab_media_unsupported(
    _: *mut c_void,
    tab: bc::bc_id,
    key_system: *const c_char,
) {
    let key_system = unsafe { CStr::from_ptr(key_system) }.to_string_lossy().into_owned();
    // check.sh reads these lines.
    match key_system.is_empty() {
        true => log!("browser: page {tab} needs nothing"),
        false => log!("browser: page {tab} needs {key_system}"),
    }
    update_shells(|shell, cx| shell.set_media_unsupported(tab, &key_system, cx));
}

/// Google's Widevine module moved along: the notes that offer it and Settings' row say where to.
/// Later, as the user's yes in a note or Settings calls back from inside their update.
unsafe extern "C" fn widevine_changed(_: *mut c_void, state: c_int) {
    // check/widevine.sh reads this line.
    log!("browser: widevine state {}", Widevine::from_core(state).name());
    update_app_later(|cx| {
        let shells: Vec<_> = WINDOWS
            .with_borrow(|windows| windows.iter().map(|window| window.shell.clone()).collect());
        for shell in shells {
            shell.update(cx, |_, cx| cx.notify()).ok();
        }
        settings::check_widevine(cx);
    });
}

unsafe extern "C" fn window_suggestions(
    _: *mut c_void,
    window: bc::bc_id,
    text: *const c_char,
    suggestions: *const bc::bc_suggestion,
    count: usize,
) {
    // The command bar shows over the window last key.
    if window != front().0 {
        return;
    }
    let string = |text: *const c_char| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();
    let suggestions = match count {
        0 => &[][..],
        _ => unsafe { std::slice::from_raw_parts(suggestions, count) },
    };
    let rows = suggestions
        .iter()
        .map(|suggestion| command_bar::Suggestion {
            url: string(suggestion.url),
            text: string(suggestion.text).into(),
            detail: string(suggestion.detail).into(),
            tab: suggestion.tab,
            completion: string(suggestion.completion),
            removable: suggestion.removable,
            keyword: string(suggestion.keyword),
            site: string(suggestion.site).into(),
            ..command_bar::Suggestion::default()
        })
        .collect();
    let text = string(text);
    update_app(|cx| command_bar::suggested(text, rows, cx));
}

unsafe extern "C" fn tab_fullscreen_changed(_: *mut c_void, tab: bc::bc_id, fullscreen: bool) {
    // check.sh times the steps from the core's "asked" line to the window's and the UI's.
    log!(
        "browser: full screen {} told at {:.1} ms",
        if fullscreen { "enter" } else { "exit" },
        browser::uptime_ms()
    );
    let Some(cx) = CX.with_borrow(Clone::clone) else {
        return;
    };
    cx.update(|cx| {
        let Some(BrowserWindow { id, shell, handle, .. }) = tab_window(tab, cx) else {
            return;
        };
        // A page leaves full screen as its tab is switched away from or closes, and only one page
        // is full screen at a time.
        let shown = shell.read_with(cx, |shell, _| shell.selected() == tab).unwrap_or(false);
        if fullscreen && !shown {
            return;
        }
        // Told before AppKit resizes a window going full screen with the page, or back, so the
        // resize lays the page out once, at its new size. Otherwise it's laid out now.
        cx.update_window(handle, |_, window, cx| {
            shell.update(cx, |shell, cx| {
                shell.set_page_fullscreen(fullscreen, window, cx);
                let viewport = window.viewport_size();
                if let Some(frame) = shell.window_page_frame(viewport) {
                    set_frames(id, shell, frame, viewport);
                }
            })
        })
        .ok();
    });
    log!(
        "browser: page {} full screen",
        if fullscreen { "entered" } else { "left" }
    );
}

/// Where Chromium keeps the profiles, and the app the user's keys. A dev build keeps a profile of
/// its own, never the owner's, since the checks delete what the last run saved. The prod build
/// keeps the user's in Application Support, under the product's name, or in Local AppData on
/// Windows.
fn profile_dir() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let name = include_str!("../branding/name").trim();
    std::env::var("BROWSER_PROFILE_DIR").unwrap_or_else(|_| {
        if cfg!(target_os = "windows") {
            let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
            if cfg!(feature = "prod") {
                format!(r"{local}\{name}\User Data")
            } else {
                format!(r"{local}\browser-app")
            }
        } else if cfg!(target_os = "linux") {
            // A Flatpak points XDG_CONFIG_HOME at its own folder, the only one it may write.
            if cfg!(feature = "prod") {
                let config = std::env::var("XDG_CONFIG_HOME")
                    .ok()
                    .filter(|dir| !dir.is_empty())
                    .unwrap_or_else(|| format!("{home}/.config"));
                format!("{config}/{name}")
            } else {
                format!("{home}/.cache/browser-app")
            }
        } else if cfg!(feature = "prod") {
            format!("{home}/Library/Application Support/{name}")
        } else {
            format!("{home}/Library/Caches/browser-app")
        }
    })
}

/// What the app gives the core to start with: Chromium's switches, from the arguments the process
/// was `given`, and the callbacks.
fn switches_and_host(given: Vec<String>) -> (Vec<CString>, bc::bc_host) {
    let profile = profile_dir();
    let mut args: Vec<String> = vec![
        format!("--user-data-dir={profile}"),
        // Chrome would copy the app bundle to check its signature against, which an
        // ad-hoc-signed build doesn't need.
        "--disable-features=MacAppCodeSignClone".into(),
        // An idle browser shouldn't wake: Chrome's stack sampling feeds metrics nobody receives,
        // and the allocator's memory reclaim backs off from every 4 s while there's little to give
        // back.
        "--disable-stack-profiler".into(),
        "--enable-features=PartitionAllocAdaptiveMemoryReclaimInterval".into(),
    ];
    // Chromium keeps its cookie encryption key in the Keychain. An ad-hoc-signed dev build asking
    // for it gets a password prompt after every build, and every request that uses cookies waits.
    // The prod build is signed alike every time (tools/install.sh), so macOS asks once.
    if !cfg!(feature = "prod") {
        args.push("--use-mock-keychain".into());
        // The same on Linux, where the key is in the desktop's keyring, and asking for it can
        // prompt too: a dev build keeps it in its profile.
        if cfg!(target_os = "linux") {
            args.push("--password-store=basic".into());
        }
    }
    // The checks' stand-in for a streaming service, a host of their server that opens in Safari's
    // engine as the listed services do (system_engine.cc). A prod build has no way to set one.
    const TEST_HOST: &str = "--streaming-test-host";
    if !cfg!(feature = "prod")
        && let Ok(host) = std::env::var("BROWSER_STREAMING_TEST_HOST")
    {
        args.push(format!("{TEST_HOST}={host}"));
    }
    // The checks' crash, which has Crashpad write a minidump (core/crash_reports.h).
    const CRASH_TEST: &str = "--crash-after-start";
    // Where the user's crash reports go if they send them, a default service URL that a fork or a
    // self-hosted collector names in branding/crash-reports. Without one the build sends none, and
    // Settings doesn't offer to. Only the prod build names it, as only it reaches users, so no dev
    // build or check sends to the product's collector; crash.sh gives the app its own.
    let collector = include_str!("../branding/crash-reports").trim();
    if cfg!(feature = "prod") && !collector.is_empty() {
        args.push(format!("--crash-upload-url={collector}"));
    }
    // The sync server the account signs in to until the user names another, a default service URL
    // a fork or a self-hosted server names in branding/sync-server (build.rs). Without one the
    // build has none, and Settings › Sync asks for the address. A --sync-url given to the app wins,
    // as a check's does: on Linux and Windows Chromium has it already, and a second would win.
    let sync_server = env!("SYNC_SERVER");
    if !sync_server.is_empty() && !given.iter().any(|arg| arg.starts_with("--sync-url=")) {
        args.push(format!("--sync-url={sync_server}"));
    }
    let given = given.into_iter().filter(|arg| {
        !cfg!(feature = "prod") || !(arg.starts_with(TEST_HOST) || arg.starts_with(CRASH_TEST))
    });
    // On Windows and Linux Chromium has started already, with what it was given.
    let chromium_started = cfg!(any(target_os = "windows", target_os = "linux"));
    let args = launch::chromium_args(given.collect(), args, chromium_started);
    let args: Vec<CString> = args
        .into_iter()
        .filter_map(|arg| CString::new(arg).ok())
        .collect();
    if std::env::var_os("BROWSER_DRIVE").is_some() {
        #[cfg(target_os = "linux")]
        drive::start();
        #[cfg(not(target_os = "linux"))]
        unsafe { platform::drive_start() };
        #[cfg(target_os = "windows")]
        unsafe { platform::drive_watch_menus(drive_menus::run) };
    }
    let host = bc::bc_host {
        launching: Some(launching),
        started: Some(started),
        stopping: Some(stopping),
        window_closed: Some(window_closed),
        private_session_ended: Some(private_session_ended),
        window_open_link: Some(window_open_link),
        space_changed: Some(space_changed),
        spaces_changed: Some(spaces_changed),
        favorites_changed: Some(favorites_changed),
        window_space_shown: Some(window_space_shown),
        window_selected: Some(window_selected),
        window_page_elsewhere: Some(window_page_elsewhere),
        tab_opened: Some(tab_opened),
        tab_closed: Some(tab_closed),
        tab_loaded: Some(tab_loaded),
        tab_state_changed: Some(tab_state_changed),
        tab_title_changed: Some(tab_title_changed),
        tab_favicon_changed: Some(tab_favicon_changed),
        tab_history: Some(tab_history),
        archive_changed: Some(archive_changed),
        page_favicon: Some(page_favicon),
        downloads_changed: Some(downloads_changed),
        download_icon: Some(download_icon),
        window_show_downloads: Some(window_show_downloads),
        extensions_changed: Some(extensions_changed),
        window_extension_popup: Some(window_extension_popup),
        window_extension_actions: Some(window_extension_actions),
        window_extension_question: Some(window_extension_question),
        window_side_panel: Some(window_side_panel),
        tab_fullscreen_changed: Some(tab_fullscreen_changed),
        tab_find_changed: Some(tab_find_changed),
        tab_permission_asked: Some(tab_permission_asked),
        tab_cast: Some(tab_cast),
        tab_site_controls: Some(tab_site_controls),
        tab_element_box: Some(tab_element_box),
        tab_link_target: Some(tab_link_target),
        tab_captured: Some(tab_captured),
        trace_recorded: Some(trace_recorded),
        tab_pictured: Some(tab_pictured),
        window_suggestions: Some(window_suggestions),
        tab_key_before_page: Some(tab_key_before_page),
        tab_key_unhandled: Some(tab_key_unhandled),
        held_keys_unclaimed: Some(held_keys_unclaimed),
        tab_media_unsupported: Some(tab_media_unsupported),
        widevine_changed: Some(widevine_changed),
        tab_page_menu: Some(tab_page_menu),
        tab_offer_changed: Some(tab_offer_changed),
        tab_passwords_changed: Some(tab_passwords_changed),
        profile_logins_changed: Some(profile_logins_changed),
        profile_autofill_changed: Some(profile_autofill_changed),
        passwords_imported: Some(passwords_imported),
        open_links: Some(open_links),
        agent_activity: Some(agent_activity),
        #[cfg(target_os = "linux")]
        window_resized: Some(window_resized),
        agent_session_ended: Some(agent_session_ended),
        tab_dialog: Some(tab_dialog),
        tab_link_to_site: Some(tab_link_to_site),
        tab_link_dragged: Some(tab_link_dragged),
        crash_report_offered: Some(crash_report_offered),
        #[cfg(target_os = "linux")]
        ui_composed: Some(ui_composed),
        #[cfg(target_os = "linux")]
        ui_committed: Some(ui_committed),
        history_added: Some(history_added),
        sync_changed: Some(sync_changed),
        sync_pairing_asked: Some(sync_pairing_asked),
        sync_devices_changed: Some(sync_devices_changed),
        tab_text: Some(tab_text),
        agent_page_done: Some(agent_page_done),
        agent_page_element: Some(agent_page_element),
        window_downloads_question: Some(window_downloads_question),
        archive_note_changed: Some(archive_note_changed),
        #[cfg(target_os = "linux")]
        window_activated: Some(window_activated),
        #[cfg(target_os = "linux")]
        window_option_tapped: Some(window_option_tapped),
        addresses_added: Some(addresses_added),
        ..Default::default()
    };
    (args, host)
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn main() {
    // Run by the version it's updating to, so macOS checks this copy before its first launch does
    // (platform/mac/updates.mm).
    if std::env::var_os("BROWSER_PREWARM").is_some() {
        return;
    }
    // The time from the process's start to here is mostly macOS's check of a version it hasn't run
    // before, which warming an update saves its first launch.
    log!("browser: main at {:.1} ms", unsafe { platform::host_since_launch() });
    if cfg!(feature = "prod") {
        // Chromium can't read the cookie key a new signature hasn't been granted, so it asks first.
        if !unsafe { platform::host_keep_keychain_key() } {
            return;
        }
        // The first launch of an update opens its release notes, over the Spaces it brings back.
        extern "C" fn update_notes(url: *const c_char) {
            let url = unsafe { CStr::from_ptr(url) }.to_string_lossy().into_owned();
            LAUNCH_LINKS.with_borrow_mut(|links| links.as_mut().map(|links| links.push(url)));
        }
        unsafe { platform::host_take_update_notes(update_notes) };
    }
    let (args, host) = switches_and_host(std::env::args().collect());
    let mut argv: Vec<*const c_char> = args.iter().map(|arg| arg.as_ptr()).collect();
    let code = unsafe { bc::bc_main(argv.len() as c_int, argv.as_mut_ptr(), &host) };
    // The quit's last stage the log can show: what's left is the process's exit (#959).
    log!("browser: quit: chromium returned {code}");
    std::process::exit(code);
}

/// On Windows this program only says where the browser is: chrome.exe runs it from Browser.dll.
#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn main() {
    log!("browser: on Windows, run chrome.exe beside Browser.dll, which windows/ builds");
    std::process::exit(1);
}

/// Sizes GPUI's window in `window` to its content, under Wayland, where it's a subsurface that only
/// the browser window's layout sizes.
#[cfg(target_os = "linux")]
unsafe extern "C" fn window_resized(_: *mut c_void, window: bc::bc_id, width: f64, height: f64) {
    if let Some(ui_view) = ui_view_of(window) {
        gpui_linux::resize_embedded(ui_view, width as f32, height as f32);
    }
}

/// The browser window became active, as GPUI's window in it doesn't hear under X11 when a click
/// in the page gives it the keyboard. After, as the core can tell of it inside the app's own call.
#[cfg(target_os = "linux")]
unsafe extern "C" fn window_activated(_: *mut c_void, window: bc::bc_id) {
    // Not in an update of the app's, as key_window_changed updates the window itself.
    if let Some(cx) = CX.with_borrow(Clone::clone) {
        let later = async move {
            if let Some(ui_view) = ui_view_of(window) {
                link_preview_window::took_keyboard(ui_view);
                key_window_changed(ui_view);
            }
        };
        cx.foreground_executor().spawn(later).detach();
    }
}

/// ⌥ was pressed alone with the page in `window` focused, which GPUI doesn't hear under X11, as
/// option_tapped is for the UI's. After, as window_activated.
#[cfg(target_os = "linux")]
unsafe extern "C" fn window_option_tapped(_: *mut c_void, window: bc::bc_id) {
    if let Some(cx) = CX.with_borrow(Clone::clone) {
        let later = async move { update_window(window, |shell, cx| shell.option_tapped(cx)) };
        cx.foreground_executor().spawn(later).detach();
    }
}

/// Under Wayland the window's input method composes `text` in the UI's text field that has the keys.
#[cfg(target_os = "linux")]
unsafe extern "C" fn ui_composed(_: *mut c_void, window: bc::bc_id, text: *const c_char) {
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    if let Some(ui_view) = ui_view_of(window) {
        gpui_linux::compose_embedded(ui_view, &text);
    }
}

/// As `ui_composed`: the input method commits `text`.
#[cfg(target_os = "linux")]
unsafe extern "C" fn ui_committed(_: *mut c_void, window: bc::bc_id, text: *const c_char) {
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    if let Some(ui_view) = ui_view_of(window) {
        gpui_linux::commit_embedded(ui_view, &text);
    }
}

#[cfg(target_os = "linux")]
fn ui_view_of(window: bc::bc_id) -> Option<*mut c_void> {
    WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == window).map(|w| w.ui_view))
}

/// On Linux this program only says where the browser is: chrome runs it from libBrowser.so.
#[cfg(target_os = "linux")]
#[allow(dead_code)]
fn main() {
    log!("browser: on Linux, run chrome beside libBrowser.so, which linux/ builds");
    std::process::exit(1);
}

/// The Windows driver's scripted input method (platform/windows/drive.cc), in GPUI's window `hwnd`:
/// marks `text` in its field with the keys, with `start..end` of it selected if `selected`, or with
/// `text` empty takes the composition out. Returns whether a field had the keys.
#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
pub extern "C" fn browser_drive_compose(
    hwnd: *mut c_void,
    text: *const c_char,
    start: usize,
    end: usize,
    selected: bool,
) -> bool {
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    gpui_windows::drive_compose(hwnd, &text, selected.then_some(start..end))
}

/// As browser_drive_compose: commits `text` in place of what was composed.
#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
pub extern "C" fn browser_drive_commit(hwnd: *mut c_void, text: *const c_char) -> bool {
    gpui_windows::drive_commit(hwnd, &unsafe { CStr::from_ptr(text) }.to_string_lossy())
}

/// What the field with the keys in GPUI's window `hwnd` has marked, in UTF-16, if anything.
#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
pub extern "C" fn browser_drive_marked(
    hwnd: *mut c_void,
    start: *mut usize,
    length: *mut usize,
) -> bool {
    let Some(marked) = gpui_windows::marked_range(hwnd) else {
        return false;
    };
    unsafe {
        *start = marked.start;
        *length = marked.len();
    }
    true
}

/// On Windows and Linux the browser is this library, which the core in Chromium's own executable
/// loads as Chromium starts, in place of a main that calls bc_main (core/public/browser_core.h).
/// What it returns lasts as long as the process.
#[cfg(any(target_os = "windows", target_os = "linux"))]
#[unsafe(no_mangle)]
pub extern "C" fn bc_app_host(argc: *mut c_int, argv: *mut *const *const c_char) -> *const bc::bc_host {
    // The arguments the process was given, from the core: Chromium on Linux has written its process
    // title over the ones std::env::args reads, joining each to the next (#605).
    let given = match unsafe { (*argc, *argv) } {
        (count, given) if count > 0 && !given.is_null() => {
            unsafe { std::slice::from_raw_parts(given, count as usize) }
        }
        _ => &[],
    };
    let given = (given.iter()).map(|&arg| unsafe { CStr::from_ptr(arg) }.to_string_lossy().into());
    let (args, host) = switches_and_host(given.collect());
    let pointers: Vec<*const c_char> = args.leak().iter().map(|arg| arg.as_ptr()).collect();
    unsafe {
        *argc = pointers.len() as c_int;
        *argv = pointers.leak().as_ptr();
    }
    Box::leak(Box::new(host))
}
