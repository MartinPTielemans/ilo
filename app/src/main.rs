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
use browser::little_launch;
use browser::shell::{
    self, Archived, BarTab, Browser, Command, Download, DownloadAction, DownloadState, Extension,
    ExtensionAction, ExtensionCommand, FilePreview, Folders, MediaAction, MenuEntry, Page,
    PagePicture, Shell, SidePanel, Split, Trouble, Volume, WindowButton,
};
use browser::text_input::{Copy, Cut, Paste, PasteAndMatchStyle, Redo, SelectAll, Undo};
use browser::theme::Appearance;
use browser::widevine::Widevine;
use browser::window_frames::{self, Frame};
use browser::{
    account, boost_editor, capture, command_bar, dialog, dock_dot, elevation::Level,
    folder_preview, hover_card, icon_picker, launch, launch_picture, link_preview, link_target,
    log, log_in, menu, offer, passwords, site_controls, theme_picker, toast, tooltip, updates,
    zoom_hud,
};
use gpui::{
    AnyWindowHandle, App, ApplicationHandle, AsyncApp, Bounds, Context, Focusable, Image,
    ImageFormat, KeyBinding, Menu, MenuItem, OsAction, Pixels, Point, SharedString, Size,
    SystemMenuType, WeakEntity, Window, WindowBackgroundAppearance, WindowHandle, WindowOptions,
    actions, point, prelude::*, px, size,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[cfg(target_os = "linux")]
#[path = "../platform/linux/active_window.rs"]
mod active_window;
mod agents_core;
mod bar_picks_host;
mod browser_windows;
mod daily_count_host;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/drive.rs"]
mod drive;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/drive_drags.rs"]
mod drive_drags;
#[cfg(target_os = "windows")]
#[path = "../platform/windows/drive_menus.rs"]
mod drive_menus;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/drive_wayland.rs"]
mod drive_wayland;
mod drop_targets;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/drops.rs"]
mod drops;
#[cfg(any(target_os = "linux", target_os = "windows"))]
#[path = "../platform/linux/emoji.rs"]
mod emoji;
#[cfg(target_os = "windows")]
#[path = "../platform/windows/fade.rs"]
mod fade;
mod find_bar;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/floats.rs"]
mod floats;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/frames.rs"]
mod frames;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/held_escape.rs"]
mod held_escape;
mod import_host;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/link_preview_window.rs"]
mod link_preview_window;
mod link_rules_host;
mod little_sites_host;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/menu_window.rs"]
mod menu_window;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/motion.rs"]
mod motion;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/needs_you.rs"]
mod needs_you;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/overlays.rs"]
mod overlays;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/page_presses.rs"]
mod page_presses;
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
mod reveal;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/reveal_window.rs"]
mod reveal_window;
mod safe_start_host;
mod settings;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/settings_window.rs"]
mod settings_window;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/shadows.rs"]
mod shadows;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/system.rs"]
mod system;
#[cfg(target_os = "linux")]
#[path = "../platform/linux/window_buttons.rs"]
mod window_buttons;
use browser_windows::*;
mod window_opening;
use window_opening::*;
mod engine;
use engine::*;
mod tabs;
use tabs::*;
mod spaces;
use spaces::*;
mod pages;
use pages::*;
mod downloads;
use downloads::*;
mod extensions;
use extensions::*;
mod profile_data;
use profile_data::*;
mod agent_events;
use agent_events::*;
mod links;
use links::*;
mod little_ilo;
mod sign_in_close;
use little_ilo::*;
mod keys;
use keys::*;
mod menu_bar;
use menu_bar::*;
mod hosts;
use hosts::*;
mod command_bar_host;
mod needs_you_host;
use command_bar_host::*;
mod link_preview_host;
use link_preview_host::*;
mod capture_host;
use capture_host::*;
mod updates_host;
use updates_host::*;
mod quitting;
mod scripting_host;
use quitting::*;
mod ad_block_host;
mod battery_saver_host;
use battery_saver_host::*;
mod other_tabs_host;
use other_tabs_host::*;
mod translate_host;
use translate_host::{tab_language, tab_translate_texts};
mod boosts_host;
use boosts_host::{tab_boost_zap_ended, tab_boost_zapped};

use browser::bc;

/// The AppKit side, in platform/mac/, or its Windows counterpart in platform/windows/.
mod platform;

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
    // The release notes of the update this launch is the first of, which the first window opens.
    static UPDATE_NOTES: RefCell<Option<String>> = const { RefCell::new(None) };
    // The account as the core last said, which it says first as it starts, before GPUI has: the
    // first window hands it to Settings.
    static SYNC_STATE: RefCell<Option<account::State>> = const { RefCell::new(None) };
    // What the menu bar was last built from, so a change to one part rebuilds it with the others.
    static MENUS: RefCell<Menus> = RefCell::default();
    // Whether a menu bar built from MENUS waits for the menu open to close.
    static MENUS_HELD: Cell<bool> = const { Cell::new(false) };
}

/// The keys area's line for `key`, which a page had, `when` the page sees it or after, gone to
/// `route`: the chord, never the character typed (logging::chord).
fn log_key(key: *const bc::bc_key, when: &str, route: &str) {
    if !browser::logging::on("keys") {
        return;
    }
    let key = unsafe { &*key };
    let name = match key.key.is_null() {
        true => Default::default(),
        false => unsafe { CStr::from_ptr(key.key) }.to_string_lossy(),
    };
    log_in!(
        "keys",
        "{} {when}: {route}",
        browser::logging::chord(&name, key.modifiers)
    );
}

/// Whether ilo keeps `key` from the page that has it, and where it went.
fn key_before_page(tab: bc::bc_id, key: *const bc::bc_key) -> (bool, &'static str) {
    #[cfg(target_os = "linux")]
    if held_escape::keeps_from_page(unsafe { &*key }) {
        return (true, "held escape");
    }
    // Any key ends the hover card, as in Dia, the page's as the UI's.
    update_app(hover_card::close);
    // The page has the keyboard, so the sidebar's ring of Focus Sidebar has gone stale.
    update_tab_window(tab, |shell, cx| shell.leave_keys("page", cx));
    if let Some(lost) = link_preview_key(tab, key, true) {
        return (lost, "link preview");
    }
    // As in Chrome, the reserved shortcuts come first, then the extensions' keys.
    if run_page_shortcut(tab, key, true) {
        return (true, "reserved shortcut");
    }
    let keystroke = keystroke(unsafe { &*key });
    // ⌘. is the agent bar's Stop while it shows one, and ⌘↩ its Take Over or Hand Back, before
    // the page could keep them.
    if update_tab_window(tab, |shell, cx| shell.run_agent_page_key(&keystroke, cx)).unwrap_or(false)
    {
        return (true, "agent bar");
    }
    // Escape ends Zap on the page, which the page doesn't see (shell/boost.rs).
    if keystroke == "escape"
        && update_tab_window(tab, |shell, cx| {
            shell.zapping(tab) && shell.escape_boost(cx)
        })
        .unwrap_or(false)
    {
        return (true, "zap");
    }
    if update_tab_window(tab, |shell, cx| {
        shell.run_extension_key(&keystroke, "page", cx)
    })
    .unwrap_or(false)
    {
        return (true, "extension");
    }
    let cx = CX.with_borrow(Clone::clone);
    // A shortcut the page gets first that it kept, pressed again at once, runs ilo's (Arc's).
    let shortcut = cx.as_ref().and_then(|cx| {
        cx.try_read_global(|keys: &shell::UserKeys, _| keys.command_for(&keystroke, false))
    });
    let repeat = unsafe { platform::host_key_repeated() };
    let now = Instant::now();
    // Esc in full screen, pressed twice or held, leaves even when the page keeps it (Arc's).
    if keystroke == "escape"
        && update_tab_window(tab, |shell, cx| {
            shell.escape_in_full_screen(repeat, now, cx)
        })
        .unwrap_or(false)
    {
        return (true, "escape in full screen");
    }
    let shortcut = shortcut.flatten().is_some();
    if SECOND_PRESS.with_borrow_mut(|second| second.press(tab, &keystroke, shortcut, repeat, now)) {
        // check.sh reads this line.
        log!("browser: second press {keystroke}");
        return (run_page_shortcut(tab, key, false), "second press");
    }
    // The keys typed after ⌘L would reach the page before it hands ⌘L back to open the bar.
    let holds = cx
        .and_then(|cx| cx.try_read_global(|keys: &shell::UserKeys, _| keys.holds_keys(&keystroke)));
    if holds == Some(true) {
        // The core says when the page has answered the key and no field opens for it, and holds
        // them itself where the host can't (bc_hold_keys).
        unsafe { bc::bc_hold_keys(cfg!(target_os = "linux")) };
        unsafe { platform::host_hold_keys() };
        #[cfg(target_os = "linux")]
        gpui_linux::hold_keys(Some(&held_keystroke(unsafe { &*key })));
        return (false, "page, holding the keys after it");
    }
    (false, "page")
}

/// Opens the windows after the first that were open when the browser last quit, each behind the
/// one before it, as they were.
fn open_saved_windows(cx: &mut App) {
    for index in 1..saved_frames().len() {
        // The window it goes behind, which it starts from as ⌘N's window does.
        let Some(above) = WINDOWS.with_borrow(|windows| windows.last().cloned()) else {
            return;
        };
        open_window_as(Some(above), None, Some(index), None, cx);
    }
}

/// A window brought back in a Space an agent left, which this launch deletes once it has moved its
/// tabs to the user's (`end_old_tasks`), as a run quit with no window left can't (#1676), shows the
/// user's Space of its profile instead. The pages the launch opens then go there and stay in front,
/// where they went to the agent's Space and the window then showed another tab.
fn leave_agent_space(window: bc::bc_id) {
    let shown = unsafe { bc::bc_window_space(window) };
    if shown == 0 || name(bc::bc_space_agent, shown).is_empty() {
        return;
    }
    let profile = unsafe { bc::bc_space_profile(shown) };
    let home = ids(bc::bc_spaces).into_iter().find(|&space| {
        name(bc::bc_space_agent, space).is_empty()
            && unsafe { bc::bc_space_profile(space) } == profile
    });
    if let Some(home) = home {
        log!("browser: window {window} left space {shown} of an agent for space {home}");
        unsafe { bc::bc_window_show_space(window, home) };
    }
}

/// Shows the Spaces as the user left them in `window`, else a first Space, and says whether it
/// brought them back. A page given in BROWSER_URL, an update's notes, and the links from other apps
/// that launched the browser, open on top either way, in this window, the one in front, the links
/// where Settings › Links sends them. With nothing to bring back and nothing to open, it's a new
/// profile's first run: `shell` plays the onboarding, then shows the welcome on the empty card. This
/// is its only way in at launch, so a launch that brings Spaces back never reads more than
/// `restored` for it. check.sh sets BROWSER_ONBOARDING to "skip" for every run but the onboarding
/// run's, which sets "light" or "dark".
fn restore_spaces(window: bc::bc_id, shell: WindowHandle<Shell>, cx: &mut App) -> bool {
    let start = Instant::now();
    let restored = unsafe { bc::bc_window_restore(window) };
    if restored {
        // check.sh reads this line.
        log!(
            "browser: restored in {:.1} ms",
            start.elapsed().as_secs_f64() * 1000.
        );
        leave_agent_space(window);
    } else {
        unsafe { bc::bc_window_show_space(window, create_space("Space 1", 0)) };
    }
    // The rules name the Spaces by their places, so they're read once the Spaces are back.
    link_rules_host::read();
    let links = LAUNCH_LINKS.take().unwrap_or_default();
    let notes = UPDATE_NOTES.take().or_else(first_launch_notes);
    let notes = browser::updates::notes_to_open(notes, std::path::Path::new(&profile_dir()));
    match std::env::var("BROWSER_URL") {
        Ok(url) => open_tab(window, &url),
        Err(_) if !restored && links.is_empty() && notes.is_none() => {
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
    // An update's notes are ilo's own page, What's New, so they open here as the command's does,
    // whatever Settings › Links says of links from other apps (#1484), and the links over them.
    if let Some(url) = notes {
        let (window, tab, space) = open_link_in(window, &url, 0);
        // check.sh reads this line.
        log!("browser: update notes {url}: tab {tab} in space {space} of window {window}");
    }
    open_links_in(window, &links);
    restored
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
            // The page is in the window's own surface, so GPUI's focus doesn't see it take the
            // keyboard, nor the Shell hear the press (page_pressed).
            page_pressed: |surface| {
                if let Some(window) = window_in(surface) {
                    ui_keyboard_left_in(window);
                    page_pressed_in(window);
                }
            },
            is_ui: |surface| {
                window_in(surface)
                    .is_some_and(|window| unsafe { bc::bc_window_keyboard_is_ui(window) })
            },
            text_input: |surface, caret| {
                if let Some(window) = window_in(surface) {
                    let caret = caret.map(|caret| {
                        (
                            f32::from(caret.origin.x),
                            f32::from(caret.origin.y),
                            f32::from(caret.size.height),
                        )
                    });
                    let (x, y, height) = caret.unwrap_or_default();
                    unsafe {
                        bc::bc_window_ui_text_input(
                            window,
                            caret.is_some(),
                            x as f64,
                            y as f64,
                            height as f64,
                        )
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
        settings::read_look(cx);
        settings::read_portrait_background(cx);
        settings::read_new_note(cx);
        settings::read_clean_links(cx);
        translate_host::read_always(cx);
        give_host_appearance(shell::app_appearance(cx));
        shell::bind_keys(cx);
        browser::text_input::bind_keys(cx);
        #[cfg(target_os = "linux")]
        held_escape::watch(wayland, cx);
        cx.set_global(menu::field_menu(Rc::new(|window, request, cx| {
            // A browser window's field by the view the host knows the window by, as its other
            // menus are, which on Linux isn't GPUI's own: Chromium's surface or the X window GPUI
            // opened in. An overlay's by its own.
            let browser = WINDOWS.with_borrow(|windows| {
                windows
                    .iter()
                    .find(|each| each.handle == window)
                    .map(|each| each.ui_view)
            });
            let view = browser.map_or_else(
                || cx.update_window(window, |_, window, _| ns_view(window)),
                Ok,
            );
            if let Ok(view) = view {
                let menu::Request {
                    items,
                    at,
                    look,
                    chosen,
                    ..
                } = request;
                show_menu(view, items, at, false, look, chosen, cx);
            }
        })));
        cx.on_action(|_: &Hide, cx| cx.hide());
        cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
        cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
        cx.on_action(|_: &Minimize, _| unsafe { platform::host_press_window_button(front().1, 1) });
        cx.on_action(|_: &CheckForUpdates, cx| check_for_updates(cx));
        cx.bind_keys([
            KeyBinding::new("cmd-h", Hide, Some(shell::KEY_CONTEXT)),
            KeyBinding::new("alt-cmd-h", HideOthers, Some(shell::KEY_CONTEXT)),
            KeyBinding::new("cmd-m", Minimize, Some(shell::KEY_CONTEXT)),
        ]);
        cx.set_menus(menus(&shell::MenuBar::default(), None, None, false));
        // check.sh reads this line, as launches are timed from it.
        log!("browser: started at {:.1} ms", unsafe {
            platform::host_since_launch()
        });
        dock_dot::open(Rc::new(DockDot), cx);
        settings::read_switches(cx);
        // The jump list's task off the Mac, which runs the browser for a new Little ilo window,
        // which the host writes as the first window opens, under its AppUserModelID.
        if let (Ok(title), Ok(switch)) = (
            CString::new(little_launch::NEW_LITTLE_WINDOW),
            CString::new(little_launch::NEW_LITTLE_SWITCH),
        ) {
            unsafe { platform::host_set_little_task(title.as_ptr(), switch.as_ptr()) };
        }
        needs_you_host::start(cx);
        daily_count_host::start(cx);
        safe_start_host::start(cx);
        open_window(None, None, cx);
        // Launched by New Incognito Window while ilo wasn't running, the private window opens
        // over the first.
        if launch_asks_private_window() {
            cx.defer(|cx| new_window_in_front(true, "launch", cx));
        }
        // A launch for a new Little ilo window, from the jump list or the launcher, once there's a
        // window for it to open from; a later one comes through the core (window_open_little).
        if little_launch::asks_for_little(std::env::args()) {
            cx.defer(|cx| new_little_window("command line", cx));
        }
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
    unsafe { platform::host_add_dock_items(dock_little, dock_chosen) };
    // AppleScript, once the windows it reads can be there.
    scripting_host::answer_scripts();
    if little_in_menu_bar() && unsafe { bc::bc_little_windows() } {
        unsafe { platform::host_set_little_status_item(Some(status_item_clicked)) };
    }
    if !FINISHED_LAUNCHING.replace(true)
        && let Some(then) = AFTER_LAUNCH.take()
    {
        update_app(then);
    }
}

/// A download a page in an agent's Space started, which the core holds for the user's answer, or
/// how one it let go on ended.
unsafe extern "C" fn agent_download(_: *mut c_void, d: *const bc::bc_agent_download) {
    use browser::agents::{Download, DownloadEnd};
    let d = unsafe { &*d };
    let text = |text: *const c_char| {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
    };
    let (id, name) = (d.download, text(d.name));
    // check.sh reads this line.
    let (tab, request, state, bytes) = (d.tab, d.request, d.state, d.bytes);
    log!("browser: agent download {id} {tab} {request} state {state} {bytes} bytes: {name}");
    let end = match d.state as u32 {
        bc::BC_DOWNLOAD_PAUSED => None,
        bc::BC_DOWNLOAD_DONE => Some(DownloadEnd::Done {
            name,
            bytes: d.bytes,
            path: text(d.path),
        }),
        bc::BC_DOWNLOAD_CANCELLED => Some(DownloadEnd::Cancelled),
        _ => Some(DownloadEnd::Failed),
    };
    let Some(end) = end else {
        let download = Download {
            agent: text(d.agent),
            tab: d.tab,
            request: d.request,
            id,
            name: text(d.name),
            url: text(d.url),
            tab_empty: d.tab_empty,
        };
        return agents_core::update(move |agents, cx| agents.download_started(download, cx));
    };
    agents_core::update(move |agents, cx| agents.download_ended(id, end, cx));
}

/// The core's answer to an agent's HTTP request (bc_agent_request).
unsafe extern "C" fn agent_request_done(
    _: *mut c_void,
    request: u64,
    response: *const bc::bc_agent_response,
) {
    let response = unsafe { &*response };
    let text = |text: *const c_char| {
        (!text.is_null()).then(|| {
            unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned()
        })
    };
    let body = match response.size {
        0 => Vec::new(),
        size => unsafe { std::slice::from_raw_parts(response.body.cast::<u8>(), size) }.to_vec(),
    };
    let response = browser::agents::request::Response {
        status: u16::try_from(response.status).unwrap_or(0),
        kind: text(response.type_).unwrap_or_default(),
        body,
        cut: response.cut,
        error: text(response.error),
    };
    log!(
        "browser: agent request {request} answered {} {}",
        response.status,
        response.body.len()
    );
    agents_core::update(move |agents, cx| agents.request_done(request, response, cx));
}

unsafe extern "C" fn launching(_: *mut c_void) {
    safe_start_host::launching();
    ad_block_host::launching();
    // While Chromium starts, about 280 ms before the first window opens (#2384). Not in a launch
    // that hands its links to the running ilo: it exits while the drivers load, which crashes it.
    #[cfg(target_os = "linux")]
    if !browser::launch::exits_at_lock(std::path::Path::new(&profile_dir())) {
        gpui_wgpu::WgpuContext::make_instance_early();
    }
    unsafe { platform::host_watch_finished_launching(finished_launching) };
    let Ok(profile) = CString::new(profile_dir()) else {
        return;
    };
    // Where the front window was, which the browser window opens at again, and whether it was
    // maximized, which only Windows saves (save_launch_frame): the launch window shows before the
    // core has read the windows it saved, whose state the browser window opens in
    // (bc_saved_window_maximized).
    let saved = std::fs::read_to_string(launch_frame_path())
        .ok()
        .and_then(|text| {
            let numbers: Vec<f64> = text
                .split_whitespace()
                .filter_map(|n| n.parse().ok())
                .collect();
            let [x, y, width, height] = numbers[..] else {
                return None;
            };
            let maximized = text.split_whitespace().any(|word| word == "maximized");
            Some((
                placed(bc::bc_rect {
                    x,
                    y,
                    width,
                    height,
                })?,
                maximized,
            ))
        });
    let (
        bc::bc_rect {
            x,
            y,
            width,
            height,
        },
        maximized,
    ) = saved.unwrap_or_else(|| {
        let (width, height) = WINDOW_SIZE;
        let (x, y) = platform::centered(width, height);
        (
            bc::bc_rect {
                x,
                y,
                width,
                height,
            },
            false,
        )
    });
    // The sidebar's picture, taken in a window near enough this one's size to show in it. A
    // maximized one is another size, kept by none.
    let taken = std::fs::read_to_string(launch_layout_path())
        .ok()
        .and_then(|line| launch_picture::read(&line));
    if let Some(taken) = taken {
        // check.sh reads this line.
        log!(
            "browser: launch picture taken at {:.0}x{:.0}",
            taken.width,
            taken.height
        );
    }
    // A picture of the column on the left, from before the sidebar moved right, is none of this
    // window's (settings::set_sidebar_right).
    let right = settings::sidebar_right_kept();
    let picture =
        taken.filter(|taken| !maximized && !right && launch_picture::shows(taken, width, height));
    let (taken_width, taken_height, foot_top) = picture.map_or((0., 0., 0.), |taken| {
        (taken.width, taken.height, taken.foot_top)
    });
    unsafe {
        platform::host_show_launch_window(
            profile.as_ptr(),
            x,
            y,
            width,
            height,
            maximized,
            taken_width,
            taken_height,
            foot_top,
        )
    };
    // check.sh reads this line.
    let state = if maximized { " maximized" } else { "" };
    log!("browser: launch window frame {x:.0} {y:.0} {width:.0} {height:.0}{state}");
    // The launch's times, from the process's start, for the performance gate.
    log!("browser: launch window at {:.1} ms", unsafe {
        platform::host_since_launch()
    });
    unsafe { platform::host_dock_dot_launch(profile.as_ptr()) };
}

/// Back from another app, such as System Settings, where the user may have changed the default
/// browser, given the app Full Disk Access or changed when scroll bars show.
extern "C" fn became_active() {
    // Later: AppKit can say so from inside the update that activated the app to show a window.
    update_app_later(|cx| {
        settings::check_default_browser(cx);
        import_host::became_active(cx);
        daily_count_host::became_active(cx);
        browser::shell::follow_scroll_bars(cx);
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

/// What GPUI's animations and the UI's own motion follow (`App::reduce_motion`). A check that
/// records motion on a runner whose system reduces it, as GitHub's hosted Mac and Windows do,
/// launches with BROWSER_MOTION=full.
fn set_reduce_motion(reduce: bool, cx: &mut App) {
    let reduce = reduce && std::env::var_os("BROWSER_MOTION").is_none_or(|motion| motion != "full");
    if reduce != cx.reduce_motion() {
        // check.sh reads this line.
        log!(
            "browser: reduce motion {}",
            if reduce { "on" } else { "off" }
        );
        cx.set_reduce_motion(reduce);
        // The pickers' grow and shrink are the platform's to draw.
        unsafe { platform::host_set_overlays_still(reduce) };
    }
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
        // ad-hoc-signed build doesn't need. And off the Mac, every switch would copy and encode
        // the tab it leaves for the hover cards of Chrome's tab strip, which ilo never shows: its
        // own hover card takes its own picture (core/capture.cc, #2377).
        "--disable-features=MacAppCodeSignClone,TabHoverCardImages".into(),
        // An idle browser shouldn't wake: Chrome's stack sampling feeds metrics nobody receives,
        // and the allocator's memory reclaim backs off from every 4 s while there's little to give
        // back.
        "--disable-stack-profiler".into(),
        // On the Mac, child processes start from Apéritif helpers, as Chrome's field trial has
        // them: those link the framework at launch, so the kernel binds its constant data clean
        // and shared, where the other helpers load it late and dyld dirties 11.7 MiB of it in each.
        // On Windows a tab's dialog from Chromium's TabDialogManager, such as 157's warning before
        // http, takes a window of its own, as the core has the older ones take
        // (PageDialogsInOwnWindow): drawn in the browser window's, it would be under the pages'
        // windows, out of sight, and a click on it would reach the page (#2651).
        format!(
            "--enable-features=PartitionAllocAdaptiveMemoryReclaimInterval{}",
            if cfg!(target_os = "macos") {
                ",AperitifHelpers"
            } else if cfg!(target_os = "windows") {
                ",TabModalUsesDesktopWidget"
            } else {
                ""
            }
        ),
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
    // A process that crashes prints its stack to the stderr the browser shares with its child
    // processes, a piece at a time, so a line of the browser's written meanwhile lands in the
    // middle of one, where the checks reading the log don't find it (#2496). check.sh keeps
    // those stacks out with BROWSER_STACK_TRACES=off; a crash still ends the process as before.
    if std::env::var("BROWSER_STACK_TRACES").is_ok_and(|traces| traces.trim() == "off") {
        args.push("--disable-in-process-stack-traces".into());
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
    // Where the daily usage count goes, which only the prod build names, unless the process was
    // given it, as a check is. The user's sync server doesn't change it.
    if let Some(switch) = daily_count_host::prepare(&given, &profile) {
        args.push(switch);
    }
    // The sync server the account signs in to until the user names another, a default service URL
    // a fork or a self-hosted server names in branding/sync-server (build.rs). Without one the
    // build has none, and Settings › Sync asks for the address. A --sync-url given to the app wins,
    // as a check's does: on Linux and Windows Chromium has it already, and a second would win.
    let sync_server = env!("SYNC_SERVER");
    if !sync_server.is_empty() && !given.iter().any(|arg| arg.starts_with("--sync-url=")) {
        args.push(format!("--sync-url={sync_server}"));
    }
    // Whether this launch holds the extensions, which the app decides from its own mark and says
    // at every launch, so a restart that gives the browser its switches again decides afresh.
    args.push(safe_start_host::prepare());
    // Which ad blocker this launch runs, from the user's choice in Settings, decided afresh at
    // every launch as safe start's is.
    let ad_blocker = ad_block_host::prepare(&given, &profile);
    args.extend(ad_blocker.switches());
    let given = given.into_iter().filter(|arg| {
        !arg.starts_with(browser::safe_start::SWITCH)
            && ad_blocker.passes(arg)
            && (!cfg!(feature = "prod")
                || !(arg.starts_with(TEST_HOST) || arg.starts_with(CRASH_TEST)))
    });
    // On Windows and Linux Chromium has started already, with what it was given.
    let chromium_started = cfg!(any(target_os = "windows", target_os = "linux"));
    let args = launch::chromium_args(given.collect(), args, chromium_started);
    let args: Vec<CString> = args
        .into_iter()
        .filter_map(|arg| CString::new(arg).ok())
        .collect();
    ad_block_host::keep_cache_in(&args);
    if std::env::var_os("BROWSER_DRIVE").is_some() {
        #[cfg(target_os = "linux")]
        drive::start();
        #[cfg(not(target_os = "linux"))]
        unsafe {
            platform::drive_start()
        };
        #[cfg(target_os = "windows")]
        unsafe {
            platform::drive_watch_menus(drive_menus::run, drive_menus::choose)
        };
    }
    let host = bc::bc_host {
        launching: Some(launching),
        started: Some(started),
        stopping: Some(stopping),
        window_closed: Some(window_closed),
        private_session_ended: Some(private_session_ended),
        window_open_link: Some(window_open_link),
        window_open_little: Some(window_open_little),
        window_reopen_little: Some(window_reopen_little),
        space_changed: Some(space_changed),
        spaces_changed: Some(spaces_changed),
        favorites_changed: Some(favorites_changed),
        window_space_shown: Some(window_space_shown),
        window_selected: Some(window_selected),
        window_page_elsewhere: Some(window_page_elsewhere),
        tab_opened: Some(tab_opened),
        tab_popup: Some(tab_popup),
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
        sidebar_backups_changed: Some(sidebar_backups_changed),
        window_extension_popup: Some(window_extension_popup),
        window_extension_actions: Some(window_extension_actions),
        window_extension_question: Some(window_extension_question),
        window_side_panel: Some(window_side_panel),
        tab_fullscreen_changed: Some(tab_fullscreen_changed),
        tab_find_changed: Some(tab_find_changed),
        tab_permission_asked: Some(tab_permission_asked),
        tab_cast: Some(tab_cast),
        tab_chooser: Some(tab_chooser),
        tab_pair: Some(tab_pair),
        tab_site_controls: Some(tab_site_controls),
        tab_element_box: Some(tab_element_box),
        tab_link_target: Some(tab_link_target),
        tab_captured: Some(tab_captured),
        trace_recorded: Some(trace_recorded),
        tab_pictured: Some(tab_pictured),
        window_suggestions: Some(window_suggestions),
        tab_key_before_page: Some(tab_key_before_page),
        tab_key_unhandled: Some(tab_key_unhandled),
        tab_press_before_page: Some(tab_press_before_page),
        tab_press_moved: Some(tab_press_moved),
        tab_press_released: Some(tab_press_released),
        tab_boost_zapped: Some(tab_boost_zapped),
        tab_boost_zap_ended: Some(tab_boost_zap_ended),
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
        launched_again: Some(launched_again),
        quit_going_ahead: Some(quit_going_ahead),
        agent_activity: Some(agent_activity),
        #[cfg(target_os = "linux")]
        window_resized: Some(window_resized),
        agent_session_ended: Some(agent_session_ended),
        tab_dialog: Some(tab_dialog),
        tab_link_to_site: Some(tab_link_to_site),
        tab_link_to_preview: Some(tab_link_to_preview),
        tab_link_dragged: Some(tab_link_dragged),
        crash_report_offered: Some(crash_report_offered),
        #[cfg(target_os = "linux")]
        ui_composed: Some(ui_composed),
        #[cfg(target_os = "linux")]
        ui_committed: Some(ui_committed),
        history_added: Some(history_added),
        history_deleted: Some(history_deleted),
        history_listed: Some(history_listed),
        sync_changed: Some(sync_changed),
        sync_pairing_asked: Some(sync_pairing_asked),
        sync_devices_changed: Some(sync_devices_changed),
        settings_synced: Some(settings_synced),
        tab_text: Some(tab_text),
        tab_language: Some(tab_language),
        tab_translate_texts: Some(tab_translate_texts),
        agent_page_done: Some(agent_page_done),
        agent_page_element: Some(agent_page_element),
        agent_download: Some(agent_download),
        agent_request_done: Some(agent_request_done),
        window_downloads_question: Some(window_downloads_question),
        archive_note_changed: Some(archive_note_changed),
        #[cfg(target_os = "linux")]
        window_activated: Some(window_activated),
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        window_option_tapped: Some(window_option_tapped),
        addresses_added: Some(addresses_added),
        battery_saver_changed: Some(battery_saver_changed),
        tab_frozen: Some(tab_frozen),
        other_tabs_changed: Some(other_tabs_changed),
        app_new_window: Some(app_new_window),
        ad_block_use: Some(ad_block_host::ad_block_use),
        ad_block_request: Some(ad_block_host::ad_block_request),
        ad_block_page: Some(ad_block_host::ad_block_page),
        ad_block_hide: Some(ad_block_host::ad_block_hide),
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
    log!("browser: main at {:.1} ms", unsafe {
        platform::host_since_launch()
    });
    if cfg!(feature = "prod") {
        // Chromium can't read the cookie key a new signature hasn't been granted, so it asks first.
        if !unsafe { platform::host_keep_keychain_key() } {
            return;
        }
    }
    // The first launch of an update opens its release notes, over the Spaces it brings back. Only
    // a build that carries Sparkle keeps them, and check.sh's links run, for its own build.
    unsafe { platform::host_take_update_notes(keep_update_notes) };
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

/// On Linux this program only says where the browser is: chrome runs it from libBrowser.so.
#[cfg(target_os = "linux")]
#[allow(dead_code)]
fn main() {
    log!("browser: on Linux, run chrome beside libBrowser.so, which linux/ builds");
    std::process::exit(1);
}

/// On Windows and Linux the browser is this library, which the core in Chromium's own executable
/// loads as Chromium starts, in place of a main that calls bc_main (core/public/browser_core.h).
/// What it returns lasts as long as the process.
#[cfg(any(target_os = "windows", target_os = "linux"))]
#[unsafe(no_mangle)]
pub extern "C" fn bc_app_host(
    argc: *mut c_int,
    argv: *mut *const *const c_char,
) -> *const bc::bc_host {
    // The arguments the process was given, from the core: Chromium on Linux has written its process
    // title over the ones std::env::args reads, joining each to the next (#605).
    let given = match unsafe { (*argc, *argv) } {
        (count, given) if count > 0 && !given.is_null() => unsafe {
            std::slice::from_raw_parts(given, count as usize)
        },
        _ => &[],
    };
    let given = (given.iter()).map(|&arg| unsafe { CStr::from_ptr(arg) }.to_string_lossy().into());
    // Chromium names the windows' X11 class and Wayland app id after this desktop entry, and the
    // desktop takes their icon from the entry it matches: ilo's, not "chromium-browser" (#1350).
    // The Flatpak's launcher sets the same. Chromium reads it later, when it opens a window.
    #[cfg(target_os = "linux")]
    if std::env::var_os("CHROME_DESKTOP").is_none_or(|desktop| desktop.is_empty()) {
        let desktop = format!("{}.desktop", include_str!("../branding/id").trim());
        unsafe { std::env::set_var("CHROME_DESKTOP", desktop) };
    }
    let (args, host) = switches_and_host(given.collect());
    let pointers: Vec<*const c_char> = args.leak().iter().map(|arg| arg.as_ptr()).collect();
    unsafe {
        *argc = pointers.len() as c_int;
        *argv = pointers.leak().as_ptr();
    }
    Box::leak(Box::new(host))
}
