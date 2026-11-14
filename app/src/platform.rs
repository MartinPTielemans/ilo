use std::ffi::{c_char, c_int, c_void};

/// Where a window `width` by `height` goes centred on the screen the windows open on, in the core's
/// terms, at whole points: the host's centre is half a point off whenever the screen's visible
/// height is odd, and the core would open the window a point taller (#1568).
pub fn centered(width: f64, height: f64) -> (f64, f64) {
    let (mut x, mut y) = (0., 0.);
    unsafe { host_centered(width, height, &mut x, &mut y) };
    browser::window_frames::whole_point((x, y))
}

unsafe extern "C" {
    #[cfg(target_os = "macos")]
    pub fn host_set_scripting(scripting: *const host_scripting);
    // Apple's Translation (platform/mac/translate.swift): the user's language, where the Mac
    // translates pages, from macOS 26.
    #[cfg(target_os = "macos")]
    pub fn host_translation_target(out: *mut c_char, capacity: usize) -> bool;
    #[cfg(target_os = "macos")]
    pub fn host_preferred_languages(out: *mut c_char, capacity: usize) -> bool;
    #[cfg(target_os = "macos")]
    pub fn host_language_name(language: *const c_char, out: *mut c_char, capacity: usize) -> bool;
    // Translates the JSON array |texts| and answers on the main thread, with |context|.
    #[cfg(target_os = "macos")]
    pub fn host_translate(
        from: *const c_char,
        to: *const c_char,
        texts: *const c_char,
        context: *mut c_void,
        done: extern "C" fn(context: *mut c_void, status: c_int, texts: *const c_char),
    );
    #[cfg(target_os = "macos")]
    pub fn host_open_translation_settings();
    #[cfg(not(target_os = "linux"))]
    pub fn drive_start();
    #[cfg(target_os = "windows")]
    pub fn drive_watch_menus(
        menus: extern "C" fn(ui_view: *mut c_void, title: *const c_char, bar: bool) -> bool,
        dialog: extern "C" fn(label: *const c_char) -> bool,
    );
    #[cfg(target_os = "linux")]
    pub fn host_choose(title: *const c_char) -> bool;
    // Through `centered` alone, which takes its point to whole points.
    fn host_centered(width: f64, height: f64, x: *mut f64, y: *mut f64);
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
    pub fn host_set_window_little(ui_view: *mut c_void, grows: bool);
    pub fn host_grow_little_window(ui_view: *mut c_void);
    pub fn host_show_window(ui_view: *mut c_void);
    pub fn host_show_window_behind(ui_view: *mut c_void, above_ui_view: *mut c_void);
    pub fn host_show_launch_window(
        profile_dir: *const c_char,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        maximized: bool,
        picture_width: f64,
        picture_height: f64,
        foot_top: f64,
    );
    pub fn host_screens(frames: *mut f64, count: usize) -> usize;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    pub fn host_app_icon(bundle_id: *const c_char, size: usize, rgba: *mut u8) -> bool;
    pub fn host_pointer(x: *mut f64, y: *mut f64) -> bool;
    pub fn host_window_frame(
        ui_view: *mut c_void,
        x: *mut f64,
        y: *mut f64,
        width: *mut f64,
        height: *mut f64,
    ) -> bool;
    pub fn host_close_launch_window();
    pub fn host_watch_finished_launching(finished: extern "C" fn());
    #[cfg(not(target_os = "windows"))]
    pub fn host_save_launch_picture(
        ui_view: *mut c_void,
        profile_dir: *const c_char,
        width: f64,
    ) -> bool;
    #[cfg(target_os = "windows")]
    pub fn host_save_launch_pixels(
        ui_view: *mut c_void,
        profile_dir: *const c_char,
        pixels: *const u8,
        columns: usize,
        rows: usize,
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
    pub fn host_set_window_shown(ui_view: *mut c_void, shown: bool);
    pub fn host_window_shown(ui_view: *mut c_void) -> bool;
    pub fn host_add_dock_items(little: extern "C" fn() -> c_int, chosen: extern "C" fn(c_int));
    pub fn host_set_little_status_item(clicked: Option<extern "C" fn()>);
    pub fn host_set_little_task(title: *const c_char, arguments: *const c_char);
    pub fn host_watch_key_window(changed: extern "C" fn(*mut c_void));
    #[cfg(target_os = "macos")]
    pub fn host_window_is_key(ui_view: *mut c_void) -> bool;
    pub fn host_watch_full_screen(changed: extern "C" fn(*mut c_void, bool));
    pub fn host_watch_window_main(changed: extern "C" fn(*mut c_void, bool));
    pub fn host_watch_swipes(swiped: extern "C" fn(*mut c_void, f64, f64, bool) -> bool);
    pub fn host_watch_option_taps(tapped: extern "C" fn(*mut c_void));
    #[cfg(not(target_os = "linux"))]
    pub fn host_watch_ui_keyboard(left: extern "C" fn(*mut c_void));
    #[cfg(not(target_os = "linux"))]
    pub fn host_watch_page_presses(pressed: extern "C" fn(*mut c_void));
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
    pub fn host_set_overlay_motion(overlay: c_int, from: f64, seconds: f64, out: f64);
    pub fn host_set_overlays_still(still: bool);
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
    pub fn host_fold_overlay(
        overlay: c_int,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        radius: f64,
        from: u32,
        to: u32,
        duration: f64,
        fade: f64,
        fade_only: bool,
    ) -> bool;
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
    pub fn host_set_pane_corner(ui_view: *mut c_void, left: f64, right: f64, top: f64);
    pub fn host_show_hud(ui_view: *mut c_void);
    pub fn host_hide_hud();
    pub fn host_add_drop_targets(view: *mut c_void);
    pub fn host_show_drop_targets(ui_view: *mut c_void);
    pub fn host_hide_drop_targets();
    pub fn host_focus(view: *mut c_void);
    pub fn host_show_window_buttons(ui_view: *mut c_void, shown: bool);
    pub fn host_set_sidebar_right(ui_view: *mut c_void, right: bool);
    pub fn host_press_window_button(ui_view: *mut c_void, button: i32);
    pub fn host_set_window_on_top(ui_view: *mut c_void, on: bool);
    #[cfg(target_os = "macos")]
    pub fn host_enter_full_screen(ui_view: *mut c_void) -> bool;
    #[cfg(target_os = "macos")]
    pub fn host_window_full_screen(ui_view: *mut c_void) -> bool;
    #[cfg(target_os = "macos")]
    pub fn host_window_on_active_space(ui_view: *mut c_void) -> bool;
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
    #[cfg(target_os = "windows")]
    pub fn host_window_restored_frame(
        ui_view: *mut c_void,
        x: *mut f64,
        y: *mut f64,
        width: *mut f64,
        height: *mut f64,
    ) -> bool;
    #[cfg(target_os = "windows")]
    pub fn host_maximize_window(ui_view: *mut c_void);
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
    pub fn host_app_active() -> bool;
    pub fn host_needs_you_badge(count: c_int);
    #[cfg(not(target_os = "linux"))]
    pub fn host_needs_you_notify(id: u64, title: *const c_char, body: *const c_char);
    #[cfg(not(target_os = "linux"))]
    pub fn host_watch_needs_you(clicked: extern "C" fn(id: u64));
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
    #[allow(clippy::too_many_arguments)]
    pub fn host_drag_tab(
        ui_view: *mut c_void,
        tab: u64,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        grab_x: f64,
        grab_y: f64,
        ended: extern "C" fn(tab: u64, how: c_int, placed: bool, x: f64, y: f64),
    ) -> bool;
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
    pub fn host_open_elsewhere(url: *const c_char);
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
    pub fn host_add_link_preview(view: *mut c_void, dismissed: extern "C" fn()) -> *mut c_void;
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
    pub fn host_show_capture(ui_view: *mut c_void, left: f64, top: f64, width: f64, height: f64);
    pub fn host_capture_camera(camera: bool);
    pub fn host_hide_capture();
    // GPUI's on Linux (platform/linux/system.rs).
    #[cfg(not(target_os = "linux"))]
    pub fn host_copy_png(png: *const u8, size: usize);
    pub fn host_desktop_picture(
        context: *mut c_void,
        png: extern "C" fn(context: *mut c_void, data: *const u8, size: usize),
    ) -> bool;
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
    pub fn host_add_foot_bar(foot_bar_view: *mut c_void);
    pub fn host_show_foot_bar(
        ui_view: *mut c_void,
        left: f64,
        right: f64,
        bottom: f64,
        width: f64,
        height: f64,
    );
    pub fn host_hide_foot_bar();
    pub fn host_add_tooltip(tooltip_view: *mut c_void, margin: f64, away: extern "C" fn());
    pub fn host_show_tooltip(ui_view: *mut c_void, overlay: c_int, left: f64, top: f64, fade: f64);
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
    pub fn host_updated_by_store() -> bool;
    #[cfg(not(target_os = "linux"))]
    pub fn host_start_updater(
        restart: extern "C" fn(),
        changed: extern "C" fn(stage: c_int, version: *const c_char),
    );
    #[cfg(not(target_os = "linux"))]
    pub fn host_nightly_build() -> bool;
    #[cfg(not(target_os = "linux"))]
    pub fn host_nightly_feed() -> bool;
    #[cfg(not(target_os = "linux"))]
    pub fn host_follow_nightly(nightly: bool);
    #[cfg(not(target_os = "linux"))]
    pub fn host_check_for_updates();
    #[cfg(not(target_os = "linux"))]
    pub fn host_check_for_updates_quietly();
    #[cfg(not(target_os = "linux"))]
    pub fn host_set_automatic_update_checks(on: bool);
    #[cfg(not(target_os = "linux"))]
    pub fn host_install_update();
    #[cfg(not(target_os = "linux"))]
    pub fn host_relaunch_into_update() -> bool;
    #[cfg(not(target_os = "linux"))]
    pub fn host_finish_update();
    #[cfg(not(target_os = "linux"))]
    pub fn host_take_update_notes(notes: extern "C" fn(url: *const c_char));
    #[cfg(target_os = "windows")]
    pub fn host_take_install(installed: extern "C" fn(build: u64, own: u64) -> bool);
}

/// Where host_start_updater's `changed` says the update stands (host.h's HOST_UPDATE_*).
pub const UPDATE_CHECKING: c_int = 1;
pub const UPDATE_CURRENT: c_int = 2;
pub const UPDATE_FOUND: c_int = 3;
pub const UPDATE_DOWNLOADING: c_int = 4;
pub const UPDATE_READY: c_int = 5;
pub const UPDATE_FAILED: c_int = 6;
pub const UPDATE_AVAILABLE: c_int = 7;

/// How the system's drag of a tab ended (host_drag_tab).
pub const TAB_TAKEN: c_int = 0;
pub const TAB_LET_GO: c_int = 1;

/// What host_take_drops hears of a drag.
pub const DRAG_OVER: c_int = 0;
pub const DRAG_DROP: c_int = 2;

/// host.h's host_scripting: what scripts ask of the app (scripting_host.rs).
#[repr(C)]
pub struct host_scripting {
    pub allowed: extern "C" fn(u64, *const c_char, *const c_char) -> bool,
    pub windows: extern "C" fn(*mut u64, usize) -> usize,
    pub window_space: extern "C" fn(u64) -> u64,
    pub window_tab: extern "C" fn(u64) -> u64,
    pub window_name: extern "C" fn(u64, *mut c_char, usize) -> usize,
    pub spaces: extern "C" fn(*mut u64, usize) -> usize,
    pub space_name: extern "C" fn(u64, *mut c_char, usize) -> usize,
    pub tabs: extern "C" fn(u64, u64, *mut u64, usize) -> usize,
    pub tab_title: extern "C" fn(u64, *mut c_char, usize) -> usize,
    pub tab_url: extern "C" fn(u64, *mut c_char, usize) -> usize,
    pub focus_space: extern "C" fn(u64, *const c_char, u64, u64, *mut c_char, usize) -> bool,
    pub select_tab: extern "C" fn(u64, *const c_char, u64, u64, *mut c_char, usize) -> bool,
    pub new_window: extern "C" fn(u64, *const c_char, *mut c_char, usize) -> u64,
    pub new_tab:
        extern "C" fn(u64, *const c_char, u64, u64, *const c_char, *mut c_char, usize) -> u64,
}

#[repr(C)]
pub struct host_dragged {
    pub url: *const c_char,
    pub file: bool,
    /// A tab of the app's that host_drag_tab drags, else 0.
    pub tab: u64,
}
