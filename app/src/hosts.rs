//! The hosts of the library's overlays and popups: dialogs, menus, the Site Controls, toasts,
//! the zoom HUD, offers, the pickers, tooltips, hover cards and the link target.

use super::*;

/// The host's overlays (platform/host.h).
pub(crate) const COMMAND_BAR: c_int = 0;

pub(crate) const ICON_PICKER: c_int = 1;

pub(crate) const THEME_PICKER: c_int = 2;

pub(crate) const FOLDER_PREVIEW: c_int = 3;

pub(crate) const BOOST_EDITOR: c_int = 4;

/// The Site Controls aren't one of the host's overlays, but its elevation call takes them too.
pub(crate) const SITE_CONTROLS: c_int = 5;

/// Shows ilo's menu of `items` at `at` in the content of `ui_view`'s window, any of the app's, or
/// of its reveal's panel if `in_reveal`, in `look`, and calls `chosen` once it closes.
pub(crate) fn show_menu(
    ui_view: *mut c_void,
    items: Vec<MenuEntry>,
    at: Point<Pixels>,
    in_reveal: bool,
    look: menu::Look,
    chosen: menu::Chosen,
    cx: &mut App,
) {
    with_overlays(
        move |cx| {
            MENU_OVER.set((ui_view, in_reveal));
            let mut area = [0f64; 4];
            unsafe { platform::host_menu_area(ui_view, in_reveal, area.as_mut_ptr()) };
            let [left, top, right, bottom] = area.map(|edge| px(edge as f32));
            let area = Bounds::from_corners(point(left, top), point(right, bottom));
            menu::show(
                menu::Request {
                    items,
                    at,
                    area,
                    look,
                    chosen,
                },
                cx,
            );
        },
        cx,
    );
}

/// The dialog's requests, to the host's dialog window.
pub(crate) struct DialogHost;

impl dialog::Host for DialogHost {
    fn show(&self, window: Option<u64>, over: Option<Bounds<Pixels>>) {
        // A window's own question goes over it, else over the one in front, as the app's do.
        let view = window.and_then(ui_view_of).unwrap_or(front().1);
        let over = rect(over.unwrap_or_default());
        unsafe { platform::host_show_dialog(view, over.x, over.y, over.width, over.height) };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_dialog() };
    }

    fn hold_keys(&self) {
        // On Linux the browser window has the keyboard back at once, as the dialog draws its
        // leaving, so nothing waits (overlays::dialog_leaving). The Mac and Windows hold the keys
        // until it has hidden.
        #[cfg(target_os = "linux")]
        crate::overlays::dialog_leaving();
        #[cfg(not(target_os = "linux"))]
        unsafe {
            platform::host_hold_keys()
        };
    }

    fn give_held_keys(&self) {
        unsafe { platform::host_release_held_keys() };
    }
}

/// The Dock icon's dot, swapped by the host (platform/mac/dock.mm).
pub(crate) struct DockDot;

impl dock_dot::Host for DockDot {
    fn show(&self, color: Option<u32>) {
        unsafe { platform::host_dock_dot(color.is_some(), color.unwrap_or(0)) };
    }
}

/// The Site Controls' requests, as calls to the core and to the host's window for them, over the
/// window in front (`front`).
pub(crate) struct SiteControlsHost {
    /// The panel's view, for its menus, once its window is open.
    pub(crate) view: Cell<*mut c_void>,
}

/// The panel's shape, for the host's blur and shadow, as x, y and radius each.
pub(crate) fn set_site_controls_shape(corners: &[(f32, f32, f32)]) {
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

    fn remove_device(&self, tab: u64, index: usize) {
        unsafe { bc::bc_tab_remove_site_device(tab, index) };
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

    fn set_boost(&self, tab: u64, on: bool) {
        update_app_later(move |cx| {
            if let Some(shell) = tab_window(tab, cx).and_then(|window| window.shell.upgrade()) {
                shell.update(cx, |shell, cx| shell.set_tab_boost_on(tab, on, cx));
            }
        });
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
                window
                    .shell
                    .update(cx, |shell, cx| shell.run(command, "site-controls", cx))
                    .ok();
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
                let url = window
                    .shell
                    .upgrade()
                    .map(|shell| shell.read(cx).page(tab).url);
                let settings = url.and_then(|url| site_controls::site_settings_url(&url));
                open_tab(
                    window.id,
                    settings.as_deref().unwrap_or("chrome://settings/content"),
                );
            }
            site_controls::Run::Extension(id, tile) => {
                window
                    .shell
                    .update(cx, |shell, cx| shell.run_extension_from(&id, tile, cx))
                    .ok();
            }
            site_controls::Run::OpenInProfile(profile) => {
                window
                    .shell
                    .update(cx, |shell, cx| shell.open_in_profile(tab, profile, cx))
                    .ok();
            }
            site_controls::Run::NewSpaceInProfile(profile) => {
                window
                    .shell
                    .update(cx, |shell, cx| shell.new_space_in_profile(profile, cx))
                    .ok();
            }
            site_controls::Run::Certificate => unsafe { bc::bc_tab_show_certificate(tab) },
            site_controls::Run::RemoveLogin(login) => {
                let profile = unsafe { bc::bc_tab_profile(tab) };
                let (message, detail) = passwords::question(&login);
                let confirmed = Box::new(move |_: &mut App| remove_login(profile, &login));
                dialog::ask(None, message, detail, "Remove", confirmed, cx);
            }
            site_controls::Run::Passwords => {
                let profile = unsafe { bc::bc_tab_profile(tab) };
                settings::show_passwords(Some(profile).filter(|&profile| profile != 0), cx);
            }
            site_controls::Run::ExtensionItem(id, item) => {
                window
                    .shell
                    .update(cx, |shell, cx| shell.extension_item(&id, item, cx))
                    .ok();
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
        let chosen = menu::picked(chosen);
        show_menu(
            self.view.get(),
            items,
            position,
            false,
            menu::Look::dark(),
            chosen,
            cx,
        );
    }

    fn tooltip(&self, tip: Option<(SharedString, tooltip::Anchor)>, cx: &mut App) {
        match tip {
            // The panel shows over the window in front, and its tooltips with it.
            Some((text, anchor)) => {
                tooltip::hover(front().0, tooltip::Tip::named(text), anchor, cx)
            }
            None => tooltip::dismiss(cx),
        }
    }

    fn has_share_sheet(&self) -> bool {
        has_share_sheet()
    }
}

thread_local! {
    /// The UI's view of the browser window ilo's menu shows over, and whether in its reveal's panel.
    pub(crate) static MENU_OVER: Cell<(*mut c_void, bool)> = const { Cell::new((std::ptr::null_mut(), false)) };
}

/// ilo's menu's requests, to the host's window for it.
pub(crate) struct MenuHost;

impl menu::Host for MenuHost {
    fn show(&self, frame: Bounds<Pixels>, panels: &[Bounds<Pixels>]) {
        // Linux's window takes the pointer on the panels alone, as its host can shape it.
        #[cfg(target_os = "linux")]
        menu_window::set_panels(panels);
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

    #[cfg(target_os = "linux")]
    fn blends(&self) -> bool {
        gpui_linux::overlays_blend()
    }
}

/// A pointer event anywhere in the app while ilo's menu shows, at `x`, `y` from its window's top
/// left. Returns whether the menu takes it.
pub(crate) extern "C" fn menu_pointer(kind: i32, x: f64, y: f64) -> bool {
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
pub(crate) extern "C" fn menu_dismissed() {
    update_app(menu::dismissed);
}

/// The driver chooses `title` in ilo's menu.
pub(crate) extern "C" fn menu_choose(title: *const c_char) -> bool {
    let title = unsafe { CStr::from_ptr(title) }
        .to_string_lossy()
        .into_owned();
    let mut chosen = false;
    update_app(|cx| chosen = menu::choose(&title, cx));
    chosen
}

/// The user left the Site Controls: a click outside them, another window or another app.
pub(crate) extern "C" fn site_controls_dismissed() {
    update_app(site_controls::dismissed);
}

thread_local! {
    /// The toast's view once its window is open, which a Share button's sheet hangs from.
    pub(crate) static TOAST_VIEW: Cell<*mut c_void> = const { Cell::new(std::ptr::null_mut()) };
}

/// The toast's requests, to the host's toast window.
pub(crate) struct ToastHost {
    /// The toast's view, for the share menu its Share button shows, once its window is open.
    pub(crate) view: Cell<*mut c_void>,
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
        unsafe { platform::host_set_toast_plate(x, y, width, height) };
    }
}

/// The foot bar's requests, to the host's foot bar window, over the browser window it names.
pub(crate) struct FootBarHost;

impl browser::foot_bar::Host for FootBarHost {
    fn show(&self, window: u64, place: browser::foot_bar::Place) {
        let Some(view) = ui_view_of(window) else {
            return;
        };
        let (width, height) = (place.size.width.into(), place.size.height.into());
        let (left, right, bottom) = (place.left.into(), place.right.into(), place.bottom.into());
        unsafe { platform::host_show_foot_bar(view, left, right, bottom, width, height) };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_foot_bar() };
    }
}

/// The zoom HUD's requests, to the host's HUD window, over the browser window it names.
pub(crate) struct HudHost;

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
pub(crate) struct OfferHost {
    pub(crate) ui_view: *mut c_void,
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

/// The icon picker's requests, to the host's overlay window, over the window that asked.
pub(crate) struct PickerHost;

impl icon_picker::Host for PickerHost {
    fn show(&self, window: u64, left: f32, top: f32) {
        let Some(view) = ui_view_of(window) else {
            return;
        };
        let (left, top, width) = (left.into(), top.into(), icon_picker::WIDTH.into());
        unsafe { platform::host_show_overlay(ICON_PICKER, view, false, left, top, width) };
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
pub(crate) fn recent_emoji_path() -> String {
    format!("{}/Recent Emoji", profile_dir())
}

pub(crate) fn recent_emoji() -> icon_picker::Recents {
    icon_picker::Recents::parse(&std::fs::read_to_string(recent_emoji_path()).unwrap_or_default())
}

/// The user left the icon picker without picking.
pub(crate) extern "C" fn icon_picker_dismissed() {
    update_app(icon_picker::dismissed);
}

pub(crate) struct FolderPreviewHost;

impl folder_preview::Host for FolderPreviewHost {
    fn show(&self, window: u64, left: f32, top: f32, height: f32, from: f32) {
        let Some(view) = ui_view_of(window) else {
            return;
        };
        let (left, top, width) = (left.into(), top.into(), folder_preview::WIDTH.into());
        let (grow, fade) = (folder_preview::IN, folder_preview::OUT);
        // At its height before it shows, so it grows from the middle of its left edge.
        unsafe {
            platform::host_set_overlay_height(FOLDER_PREVIEW, height.into());
            platform::host_set_overlay_growth(FOLDER_PREVIEW, 0., (height / 2.).into());
            platform::host_set_overlay_motion(
                FOLDER_PREVIEW,
                from.into(),
                grow.as_secs_f64(),
                fade.as_secs_f64(),
            );
            platform::host_show_overlay(FOLDER_PREVIEW, view, false, left, top, width);
        }
    }

    fn hide(&self) {
        unsafe { platform::host_hide_overlay(FOLDER_PREVIEW) };
    }
}

pub(crate) extern "C" fn folder_preview_dismissed() {
    update_app(folder_preview::dismissed);
}

/// The theme picker's requests, to the host's overlay window, over the window that asked.
pub(crate) struct ThemeHost;

impl theme_picker::Host for ThemeHost {
    fn show(&self, window: u64, left: f32, top: f32, corners: &[(f32, f32, f32)]) {
        let Some(view) = ui_view_of(window) else {
            return;
        };
        let shape: Vec<f64> = corners
            .iter()
            .flat_map(|&(x, y, radius)| [x, y, radius].map(f64::from))
            .collect();
        // It grows in from its first corner: the arrow's tip, which moves along its side to point
        // at what opened it, or with no arrow the card's top left, against the sidebar.
        let (x, y, _) = corners[0];
        let (left, top, width) = (left.into(), top.into(), theme_picker::WIDTH.into());
        unsafe {
            platform::host_set_overlay_shape(THEME_PICKER, shape.as_ptr(), shape.len() / 3);
            platform::host_set_overlay_growth(THEME_PICKER, x.into(), y.into());
            platform::host_show_overlay(THEME_PICKER, view, false, left, top, width);
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
pub(crate) extern "C" fn theme_picker_dismissed() {
    update_app(theme_picker::dismissed);
}

/// The Boost editor's requests, to the host's overlay window, over the window that asked, as the
/// theme picker's.
pub(crate) struct BoostEditorHost;

impl boost_editor::Host for BoostEditorHost {
    fn show(&self, window: u64, left: f32, top: f32, corners: &[(f32, f32, f32)]) {
        let Some(view) = ui_view_of(window) else {
            return;
        };
        let shape: Vec<f64> = corners
            .iter()
            .flat_map(|&(x, y, radius)| [x, y, radius].map(f64::from))
            .collect();
        let (x, y, _) = corners[0];
        let (left, top, width) = (left.into(), top.into(), boost_editor::WIDTH.into());
        unsafe {
            platform::host_set_overlay_shape(BOOST_EDITOR, shape.as_ptr(), shape.len() / 3);
            platform::host_set_overlay_growth(BOOST_EDITOR, x.into(), y.into());
            platform::host_show_overlay(BOOST_EDITOR, view, false, left, top, width);
        }
    }

    fn resize(&self, height: f32, corners: &[(f32, f32, f32)]) {
        let shape: Vec<f64> = corners
            .iter()
            .flat_map(|&(x, y, radius)| [x, y, radius].map(f64::from))
            .collect();
        unsafe {
            platform::host_set_overlay_shape(BOOST_EDITOR, shape.as_ptr(), shape.len() / 3);
            platform::host_set_overlay_height(BOOST_EDITOR, height.into());
        }
    }

    fn hide(&self) {
        unsafe { platform::host_hide_overlay(BOOST_EDITOR) };
    }
}

/// The user left the Boost editor, such as with a click outside it.
pub(crate) extern "C" fn boost_editor_dismissed() {
    update_app(boost_editor::dismissed);
}

/// The tooltip's requests, to the host's tooltip window, over the window the pointer is in.
pub(crate) struct TooltipHost;

impl tooltip::Host for TooltipHost {
    fn show(&self, over: tooltip::Over, left: f32, top: f32) {
        let (view, overlay) = match over {
            tooltip::Over::Window(window) => (ui_view_of(window), -1),
            // The overlays show over the window in front.
            tooltip::Over::Overlay(overlay) => (Some(front().1), overlay),
        };
        let Some(view) = view.filter(|view| !view.is_null()) else {
            return;
        };
        let fade = tooltip::FADE_IN.as_secs_f64();
        unsafe { platform::host_show_tooltip(view, overlay, left.into(), top.into(), fade) };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_tooltip(tooltip::FADE_OUT.as_secs_f64()) };
    }
}

/// The window lost the keyboard, or the app stopped being the active one.
pub(crate) extern "C" fn tooltip_away() {
    // Later: the window resigns key from inside the update that opened another, such as Settings.
    update_app_later(tooltip::dismiss);
}

/// The hover card's requests, to the host's card window, over the browser window whose row it's
/// for.
pub(crate) struct CardHost;

impl hover_card::Host for CardHost {
    fn show(&self, window: u64, left: f32, top: f32, width: f32, height: f32) {
        let Some(view) = ui_view_of(window) else {
            return;
        };
        let (left, top, width, height) = (left.into(), top.into(), width.into(), height.into());
        unsafe { platform::host_show_hover_card(view, left, top, width, height) };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_hover_card() };
    }
}

/// The window lost the keyboard, the app stopped being the active one, or a press or a scroll
/// landed outside the card.
pub(crate) extern "C" fn card_away() {
    // Later, as tooltip_away.
    update_app_later(hover_card::close);
}

/// The system is short of memory: the hover cards' pictures, which the core can take again, go.
pub(crate) extern "C" fn memory_pressed() {
    update_app(hover_card::forget_all);
}

/// The link target's requests, to the host's link target window, over the window whose page has
/// the link.
pub(crate) struct LinkTargetHost;

impl link_target::Host for LinkTargetHost {
    fn show(&self, window: u64, left: f32, top: f32) {
        let Some(view) = ui_view_of(window) else {
            return;
        };
        let fade = link_target::FADE_IN.as_secs_f64();
        unsafe { platform::host_show_link_target(view, left.into(), top.into(), fade) };
    }

    fn hide(&self) {
        unsafe { platform::host_hide_link_target(link_target::FADE_OUT.as_secs_f64()) };
    }

    fn pressed(&self) -> bool {
        unsafe { platform::host_mouse_down() }
    }
}

/// The pointer moved, at (`x`, `y`) in the window the link target shows over, while it shows.
pub(crate) extern "C" fn link_pointer(x: f64, y: f64) {
    update_app(|cx| {
        if let Some(window) = link_target::over(cx) {
            update_window_in(window, cx, |shell, cx| {
                shell.link_pointer(x as f32, y as f32, cx)
            });
        }
    });
}

/// The pointer is on a link in `tab`'s page, or has left it and `url` is "".
pub(crate) unsafe extern "C" fn tab_link_target(
    _: *mut c_void,
    tab: bc::bc_id,
    url: *const c_char,
    domain_begin: usize,
    domain_end: usize,
) {
    let url = unsafe { CStr::from_ptr(url) }
        .to_string_lossy()
        .into_owned();
    // check.sh reads this line.
    log!(
        "browser: link target {}",
        if url.is_empty() { "none" } else { url.as_str() }
    );
    let target = (!url.is_empty()).then(|| link_target::Target {
        text: url.into(),
        domain: domain_begin..domain_end,
    });
    // After the update under way, if any: the core also says it when a page navigates, inside the
    // call that navigated.
    update_app_later(move |cx| {
        if let Some(window) = tab_window(tab, cx) {
            window
                .shell
                .update(cx, |shell, cx| shell.link_target_changed(tab, target, cx))
                .ok();
        }
    });
}

/// After a crash, with Send Crash Reports off: a toast asks whether to send its report, until the
/// user answers, rather than an alert in the way of the windows coming back.
pub(crate) unsafe extern "C" fn crash_report_offered(_: *mut c_void) {
    let button = |label: &str, name, send| toast::Button {
        label: toast::Label::Text(label.to_owned().into()),
        name,
        run: Rc::new(move |_, _| unsafe { bc::bc_answer_crash_report(send) }),
    };
    let buttons = vec![
        button("Send Report", "send", true),
        button("Don't Send", "don't send", false),
    ];
    let text = concat!(env!("APP_NAME"), " quit unexpectedly.");
    update_app(move |cx| {
        with_overlays(move |cx| toast::show_until_answered(text, buttons, cx), cx)
    });
}
