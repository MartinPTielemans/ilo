//! The browser windows the app keeps, and the ways to reach a window's shell and view from
//! the core's ids, the host's views and GPUI's updates.

use super::*;

/// A browser window: the core's, with the view the UI draws in and the shell drawn there.
#[derive(Clone)]
pub(crate) struct BrowserWindow {
    pub(crate) id: bc::bc_id,
    pub(crate) ui_view: *mut c_void,
    /// GPUI's own X11 window, which it makes inside `ui_view` and names a press in.
    #[cfg(target_os = "linux")]
    pub(crate) view: *mut c_void,
    pub(crate) shell: WeakEntity<Shell>,
    pub(crate) handle: AnyWindowHandle,
    /// Its own overlays' windows, such as the find bar's, which close with it.
    pub(crate) children: Vec<AnyWindowHandle>,
    /// A private window's: it shows a Space of its own, in an off-the-record session.
    pub(crate) private: bool,
    /// A Little ilo window's: it shows a Space of its own, for the user's Space here, which ⌘O
    /// moves its page to.
    pub(crate) little: Option<bc::bc_id>,
    /// A Blank Window's: it shows a Space of its own, unsaved, in the user's profile.
    pub(crate) blank: bool,
    /// The user's Space it showed last, not an agent's, or 0, for a link from another app.
    pub(crate) users_space: bc::bc_id,
    /// The link from another app a Little ilo window opened for, which brings it back when another
    /// app sends it again, as Arc's does.
    pub(crate) link: Option<String>,
}

impl BrowserWindow {
    /// Whether it shows the user's Spaces, as no private, Little ilo or Blank Window does.
    pub(crate) fn regular(&self) -> bool {
        !self.private && self.little.is_none() && !self.blank
    }
}

/// A window that shows a Space of its own, which the core makes with it.
#[derive(Clone, Copy)]
pub(crate) enum Own {
    /// ⇧⌘N's private window, in this profile's off-the-record session.
    Private(bc::bc_id),
    /// Little ilo's, in `profile`, for the user's Space `home`, which ⌘O moves its page to.
    Little { profile: bc::bc_id, home: bc::bc_id },
    /// ⌃⌘N's Blank Window, in `profile`: its tabs aren't kept or shown in other windows.
    Blank(bc::bc_id),
}

/// The core's id and the UI's view of the window that the overlays every window shares show over,
/// such as the command bar: the browser window last key. 0 and null before one opens.
pub(crate) fn front() -> (bc::bc_id, *mut c_void) {
    WINDOWS.with_borrow(|windows| {
        windows.first().map_or((0, std::ptr::null_mut()), |window| {
            (window.id, window.ui_view)
        })
    })
}

/// The UI's view of browser window `window`, such as the one a hover-driven overlay is for, which
/// it shows over whether or not it's in front (#1432). None once it has closed.
pub(crate) fn ui_view_of(window: bc::bc_id) -> Option<*mut c_void> {
    WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == window).map(|w| w.ui_view))
}

/// A browser window has gone full screen or left it, once macOS's transition is over. Later, as
/// AppKit can say so from inside an update of ours, such as one that closes the window.
pub(crate) extern "C" fn full_screen_changed(ui_view: *mut c_void, entered: bool) {
    let id = window_with_view(ui_view);
    update_app_later(move |cx| {
        update_window_in(id, cx, |shell, cx| shell.set_full_screen(entered, cx));
    });
}

pub(crate) extern "C" fn swiped(ui_view: *mut c_void, x: f64, y: f64, back: bool) -> bool {
    let at = point(px(x as f32), px(y as f32));
    update_window(window_with_view(ui_view), |shell, cx| {
        shell.swiped(at, back, cx)
    })
    .unwrap_or(false)
}

/// ⌥ was pressed and let go alone in the window whose UI draws in `ui_view`.
pub(crate) extern "C" fn option_tapped(ui_view: *mut c_void) {
    update_window(window_with_view(ui_view), |shell, cx| {
        shell.option_tapped(cx)
    });
}

/// The window whose UI draws in `ui_view`, or 0.
pub(crate) fn window_with_view(ui_view: *mut c_void) -> bc::bc_id {
    WINDOWS.with_borrow(|windows| {
        windows
            .iter()
            .find(|window| window.ui_view == ui_view)
            .map_or(0, |window| window.id)
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
pub(crate) extern "C" fn key_window_changed(ui_view: *mut c_void) {
    let moved = WINDOWS.with_borrow_mut(|windows| {
        let index = windows
            .iter()
            .position(|window| window.ui_view == ui_view)?;
        (index > 0).then(|| {
            let window = windows.remove(index);
            windows.insert(0, window.clone());
            window
        })
    });
    if let Some(window) = moved {
        log!("browser: key window {}", window.id);
        update_app_later(shell::window_became_key(window.shell));
    }
}

/// The browser window drawn in `ui_view` became the one the user is in, or stopped being, which
/// its header's glyphs show (host_watch_window_main). The host can tell from inside the update that
/// opened or closed a window, so the shell hears once that's over.
pub(crate) extern "C" fn window_main_changed(ui_view: *mut c_void, main: bool) {
    update_app_later(move |cx| {
        let id = window_with_view(ui_view);
        if id == 0 {
            return;
        }
        // check.sh reads this line.
        log!("browser: window {id} main {main}");
        _ = shell_with_view(ui_view).update(cx, |shell, cx| shell.set_window_main(main, cx));
        let handle = WINDOWS.with_borrow(|windows| {
            let window = windows.iter().find(|window| window.ui_view == ui_view)?;
            Some(window.handle)
        });
        // Once the frame that drew it is on screen, which the next frame's start says, for
        // check.sh's capture.
        if let Some(handle) = handle {
            _ = handle.update(cx, |_, window, _| {
                window.on_next_frame(move |window, _| {
                    window.on_next_frame(move |_, _| log!("browser: window {id} drew main {main}"));
                    window.refresh();
                });
            });
        }
    });
}

/// The UI of the window drawn in `ui_view` lost the keyboard to its page, or an overlay took it,
/// which GPUI's focus doesn't see: a field in its sidebar ends, as for a click elsewhere (#1414).
pub(crate) extern "C" fn ui_keyboard_left(ui_view: *mut c_void) {
    ui_keyboard_left_in(window_with_view(ui_view));
}

/// As `ui_keyboard_left`, for the window `id`. After the event: the host can tell in a call the UI
/// made, such as one giving the page the keyboard.
pub(crate) fn ui_keyboard_left_in(id: bc::bc_id) {
    // As the press comes, before what's queued behind it opens a field (#2260).
    let opened = crate::shell::fields_opened();
    let Some((handle, shell)) = WINDOWS.with(|windows| {
        let windows = windows.try_borrow().ok()?;
        let window = windows.iter().find(|window| window.id == id)?;
        Some((window.handle, window.shell.clone()))
    }) else {
        return;
    };
    update_app_later(move |cx| {
        handle
            .update(cx, |_, window, cx| {
                shell
                    .update(cx, |shell, cx| shell.keyboard_left(opened, window, cx))
                    .ok()
            })
            .ok();
    });
}

/// The user pressed a mouse button in a page of the window drawn in `ui_view`, which an agent's
/// page shown there takes as the user taking it over (Shell::page_pressed).
pub(crate) extern "C" fn page_pressed(ui_view: *mut c_void) {
    page_pressed_in(window_with_view(ui_view));
}

/// As `page_pressed`, for the window `id`: after the press, which the page has.
pub(crate) fn page_pressed_in(id: bc::bc_id) {
    let Some(shell) = WINDOWS.with(|windows| {
        let windows = windows.try_borrow().ok()?;
        let window = windows.iter().find(|window| window.id == id)?;
        Some(window.shell.clone())
    }) else {
        return;
    };
    update_app_later(move |cx| {
        shell.update(cx, |shell, cx| shell.page_pressed(cx)).ok();
    });
}

/// Changes every window's shell, from a core callback about what they all show, such as the
/// Spaces and their tabs.
pub(crate) fn update_shells(change: impl Fn(&mut Shell, &mut Context<Shell>)) {
    let Some(cx) = CX.with_borrow(Clone::clone) else {
        return;
    };
    cx.update(|cx| update_shells_in(cx, change));
}

/// Changes every window's shell, as `update_shells`, in an update under way.
pub(crate) fn update_shells_in(cx: &mut App, change: impl Fn(&mut Shell, &mut Context<Shell>)) {
    let shells: Vec<_> =
        WINDOWS.with_borrow(|windows| windows.iter().map(|window| window.shell.clone()).collect());
    for shell in shells {
        shell.update(cx, &change).ok();
    }
}

/// Changes the shell of the window `id`, from a core callback about that window, and returns what
/// the change does, unless there's no such window.
pub(crate) fn update_window<R>(
    id: bc::bc_id,
    change: impl FnOnce(&mut Shell, &mut Context<Shell>) -> R,
) -> Option<R> {
    let shell = WINDOWS.with_borrow(|windows| {
        windows
            .iter()
            .find(|window| window.id == id)
            .map(|window| window.shell.clone())
    })?;
    let cx = CX.with_borrow(Clone::clone)?;
    cx.update(|cx| shell.update(cx, change).ok())
}

/// Changes the shell of the window `id`, as `update_window`, in an update under way.
pub(crate) fn update_window_in<R>(
    id: bc::bc_id,
    cx: &mut App,
    change: impl FnOnce(&mut Shell, &mut Context<Shell>) -> R,
) -> Option<R> {
    let shell = WINDOWS.with_borrow(|windows| {
        windows
            .iter()
            .find(|window| window.id == id)
            .map(|window| window.shell.clone())
    })?;
    shell.update(cx, change).ok()
}

pub(crate) fn tab_window(tab: u64, cx: &App) -> Option<BrowserWindow> {
    let core = unsafe { bc::bc_tab_window(tab) };
    WINDOWS.with_borrow(|windows| {
        let shows = |window: &&BrowserWindow| {
            window
                .shell
                .upgrade()
                .is_some_and(|shell| shell.read(cx).shows(tab))
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
pub(crate) fn update_tab_window<R>(
    tab: u64,
    change: impl FnOnce(&mut Shell, &mut Context<Shell>) -> R,
) -> Option<R> {
    let cx = CX.with_borrow(Clone::clone)?;
    cx.update(|cx| tab_window(tab, cx)?.shell.update(cx, change).ok())
}

/// Changes the UI outside the shells, from a core callback.
pub(crate) fn update_app(change: impl FnOnce(&mut App)) {
    if let Some(cx) = CX.with_borrow(Clone::clone) {
        cx.update(change);
    }
}

/// Changes the app once the update under way, if any, is over, for a core callback that can come
/// from inside the call that asked for it.
pub(crate) fn update_app_later(change: impl FnOnce(&mut App) + 'static) {
    if let Some(cx) = CX.with_borrow(Clone::clone) {
        let executor = cx.foreground_executor().clone();
        executor.spawn(async move { cx.update(change) }).detach();
    }
}

/// The view of a window the host fades, as the tooltip's: on Windows the app paints its fades.
pub(crate) fn fading_view(window: &mut Window) -> *mut c_void {
    let view = ns_view(window);
    #[cfg(target_os = "windows")]
    fade::attach(window, view);
    view
}

/// The window's NSView, for the host.
pub(crate) fn ns_view(window: &Window) -> *mut c_void {
    match HasWindowHandle::window_handle(window).map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::AppKit(handle)) => handle.ns_view.as_ptr(),
        Ok(RawWindowHandle::Win32(handle)) => handle.hwnd.get() as *mut c_void,
        Ok(RawWindowHandle::Xcb(handle)) => handle.window.get() as usize as *mut c_void,
        Ok(RawWindowHandle::Wayland(handle)) => handle.surface.as_ptr(),
        _ => panic!("the window has no native view"),
    }
}

/// Tells the core where the window's page goes and the radius of its corners
/// (`Shell::page_frame`).
pub(crate) fn set_page_frame(window: bc::bc_id, (frame, radius): (Bounds<Pixels>, f32)) {
    unsafe { bc::bc_window_set_page_frame(window, rect(frame), f64::from(radius)) };
}

/// Tells the core where the window's page, at `page`, and its side panel's page go, in a window
/// whose content is `viewport`, as it resizes or a page goes full screen between the shell's draws.
pub(crate) fn set_frames(
    window: bc::bc_id,
    shell: &Shell,
    page: (Bounds<Pixels>, f32),
    viewport: Size<Pixels>,
) {
    set_page_frame(window, page);
    let panel = shell.side_panel_page(viewport).unwrap_or_default();
    unsafe { bc::bc_window_set_side_panel_frame(window, rect(panel)) };
}

pub(crate) fn rect(bounds: Bounds<Pixels>) -> bc::bc_rect {
    bc::bc_rect {
        x: f64::from(bounds.origin.x),
        y: f64::from(bounds.origin.y),
        width: f64::from(bounds.size.width),
        height: f64::from(bounds.size.height),
    }
}

/// Sizes GPUI's window in `window` to its content, under Wayland, where it's a subsurface that only
/// the browser window's layout sizes.
#[cfg(target_os = "linux")]
pub(crate) unsafe extern "C" fn window_resized(
    _: *mut c_void,
    window: bc::bc_id,
    width: f64,
    height: f64,
) {
    // A window the launch puts back full screen can be told its state before it has a size, 0x0,
    // and GPUI's viewport at 0x0 is a protocol error, which ends the Wayland connection. Its size
    // comes with its bounds.
    let Some(ui_view) = ui_view_of(window).filter(|_| width >= 1. && height >= 1.) else {
        return;
    };
    let again = frames::size_of(ui_view) == Some([width, height]);
    gpui_linux::resize_embedded(ui_view, width as f32, height as f32);
    frames::sized(ui_view, [width, height]);
    // The core tells the size again once the window has gone full screen or maximized, or back.
    // At the size GPUI's window has, as a window already its screen's size goes or one whose size
    // came before its state, no resize comes to ask and to lay the page out, so it's done here, as
    // the resize does (#1220).
    if again {
        let viewport = gpui::size(gpui::px(width as f32), gpui::px(height as f32));
        update_app_later(move |cx| {
            update_window_in(window, cx, |shell, cx| {
                shell.bounds_changed(cx);
                if let Some(frame) = shell.window_page_frame(viewport) {
                    set_frames(window, shell, frame, viewport);
                    shell.full_screen_step("laid", viewport);
                }
            });
        });
    }
}

/// The browser window became active, as GPUI's window in it doesn't hear under X11 when a click
/// in the page gives it the keyboard. After, as the core can tell of it inside the app's own call.
#[cfg(target_os = "linux")]
pub(crate) unsafe extern "C" fn window_activated(_: *mut c_void, window: bc::bc_id) {
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

/// ⌥ was pressed alone in `window`: with its page focused, which GPUI doesn't hear under X11, as
/// option_tapped is for the UI's, or on Windows with either focused. After, as window_activated.
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) unsafe extern "C" fn window_option_tapped(_: *mut c_void, window: bc::bc_id) {
    if let Some(cx) = CX.with_borrow(Clone::clone) {
        let later = async move { update_window(window, |shell, cx| shell.option_tapped(cx)) };
        cx.foreground_executor().spawn(later).detach();
    }
}
