//! Opening a browser window and the shell in it, where it opens, the overlays' windows it makes,
//! and its closing.

use super::*;

pub(crate) const WINDOW_SIZE: (f64, f64) = (1280., 800.);

/// Opens a browser window and returns its id, with `ui_view` filling it under the page: the first
/// at the frame the front window had when the browser last quit, saved window `saved` behind the
/// others at its own (bc_window_open_saved), each kept on screen, else centered on the main
/// display, or cascaded from the one of `after` and as big, or at `at`, kept on a screen. With
/// `own`, it's a private window in that profile's off-the-record session, or a Little ilo window,
/// where Little ilo's open.
pub(crate) fn open_browser_window(
    ui_view: *mut c_void,
    after: Option<*mut c_void>,
    own: Option<Own>,
    saved: Option<usize>,
    at: Option<bc::bc_rect>,
) -> Option<bc::bc_id> {
    let frames = if after.is_none() || saved.is_some() {
        saved_frames()
    } else {
        Vec::new()
    };
    let frame = match own {
        // A page's pop-up's Little ilo window is where its page asked, else Little ilo's own.
        Some(Own::Little { .. }) => Some(at.unwrap_or_else(little_frame)),
        _ if at.is_some() => at.map(|at| placed(at).unwrap_or(at)),
        _ => frames.get(saved.unwrap_or(0)).copied().and_then(placed),
    };
    let bounds = frame.unwrap_or_else(|| {
        let (mut width, mut height) = WINDOW_SIZE;
        let (mut x, mut y) = (0., 0.);
        match after {
            Some(after) => unsafe {
                platform::host_cascaded(after, &mut x, &mut y, &mut width, &mut height)
            },
            None => (x, y) = platform::centered(width, height),
        }
        bc::bc_rect {
            x,
            y,
            width,
            height,
        }
    });
    let window = match (own, saved) {
        (Some(Own::Private(profile)), _) => unsafe {
            bc::bc_window_open_private(bounds, ui_view, profile)
        },
        (Some(Own::Little { profile, .. }), _) => unsafe {
            bc::bc_window_open_little(bounds, ui_view, profile)
        },
        (Some(Own::Blank(profile)), _) => unsafe {
            bc::bc_window_open_blank(bounds, ui_view, profile)
        },
        (None, Some(index)) => unsafe { bc::bc_window_open_saved(index, bounds, ui_view) },
        (None, None) => unsafe { bc::bc_window_open(bounds, ui_view) },
    };
    (window != 0).then_some(window)
}

/// Maximizes the browser window `id` of `ui_view`, which then un-maximizes to the frame it has, as
/// Windows' title bar and Linux's third window button do. The Mac's zoom is a frame, which the core
/// saves (bc_saved_window_maximized).
#[allow(unused_variables)]
fn maximize(id: bc::bc_id, ui_view: *mut c_void) {
    #[cfg(target_os = "windows")]
    unsafe {
        platform::host_maximize_window(ui_view)
    };
    #[cfg(target_os = "linux")]
    unsafe {
        bc::bc_window_toggle_maximized(id)
    };
}

/// The frames of the windows open when the browser last quit, front to back.
pub(crate) fn saved_frames() -> Vec<bc::bc_rect> {
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
pub(crate) fn placed(saved: bc::bc_rect) -> Option<bc::bc_rect> {
    let saved = Frame {
        x: saved.x,
        y: saved.y,
        width: saved.width,
        height: saved.height,
    };
    let frame = window_frames::place(saved, &screens())?;
    Some(bc::bc_rect {
        x: frame.x,
        y: frame.y,
        width: frame.width,
        height: frame.height,
    })
}

/// The visible frames of the screens a window may open on, the main one first (host_screens).
pub(crate) fn screens() -> Vec<Frame> {
    let count = unsafe { platform::host_screens(std::ptr::null_mut(), 0) };
    let mut screens = vec![0.; count * 4];
    let count = unsafe { platform::host_screens(screens.as_mut_ptr(), count) };
    (screens.as_chunks::<4>().0.iter())
        .take(count)
        .map(|&[x, y, width, height]| Frame {
            x,
            y,
            width,
            height,
        })
        .collect()
}

/// Logs where the window is, for check.sh.
pub(crate) fn log_frame(id: bc::bc_id, ui_view: *mut c_void) {
    let (mut x, mut y, mut width, mut height) = (0., 0., 0., 0.);
    if unsafe { platform::host_window_frame(ui_view, &mut x, &mut y, &mut width, &mut height) } {
        log!("browser: window {id} frame {x:.0} {y:.0} {width:.0} {height:.0}");
    }
}

/// Opens a browser window after `id`'s, for ⇧↩ in the command bar: as ⌘N does, or from a private
/// window as ⇧⌘N does, so what was typed there stays off the record. Returns it.
pub(crate) fn open_window_after(id: bc::bc_id, cx: &mut App) -> Option<bc::bc_id> {
    let from = WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == id).cloned())?;
    // The profile of the Space a private window shows, as ⇧⌘N takes it.
    let private = from
        .private
        .then(|| unsafe { bc::bc_space_profile(bc::bc_window_space(id)) });
    if private == Some(0) {
        return None;
    }
    open_window(Some(from), private.map(Own::Private), cx)
}

thread_local! {
    /// Whether the window opened last came on screen with its first frame drawn.
    static DRAWN_BEFORE_SHOWN: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// When the window opened last came on screen with its first frame drawn, so that frame was on
/// screen as it was shown, if it did.
pub(crate) fn drawn_before_shown() -> Option<Instant> {
    DRAWN_BEFORE_SHOWN.get()
}

/// Opens a browser window and the shell inside it under the page, and returns it. The first opens
/// the overlays every window shares and the Spaces as the user left them. Another, from `from`,
/// starts with its shell's state and shows its Space with no page, as ⌘N's window does. With
/// `own`, it's ⇧⌘N's private window or a Little ilo window, which shows a Space of its own
/// instead, with no page.
pub(crate) fn open_window(
    from: Option<BrowserWindow>,
    own: Option<Own>,
    cx: &mut App,
) -> Option<bc::bc_id> {
    open_window_as(from, own, None, None, cx)
}

/// open_window, as ⌘N does, at `at` if given, kept on a screen, for a tab dragged out of `from`.
pub(crate) fn open_window_at(
    from: Option<BrowserWindow>,
    at: Option<bc::bc_rect>,
    cx: &mut App,
) -> Option<bc::bc_id> {
    open_window_as(from, None, None, at, cx)
}

/// open_window, or with `saved`, the saved window after the first the launch brings back
/// (bc_window_open_saved): behind `from` and not key, starting with its shell's state as ⌘N's
/// window does, but showing the Space it showed, with its page.
pub(crate) fn open_window_as(
    from: Option<BrowserWindow>,
    own: Option<Own>,
    saved: Option<usize>,
    at: Option<bc::bc_rect>,
    cx: &mut App,
) -> Option<bc::bc_id> {
    #[cfg(not(target_os = "linux"))]
    let ui_view = unsafe { platform::host_ui_view() };
    // GPUI's own X11 window, which its window opens in, as the view the core holds.
    #[cfg(target_os = "linux")]
    let ui_view = gpui_linux::new_host_window().unwrap_or(0) as usize as *mut c_void;
    let after = from.as_ref().map(|from| from.ui_view);
    let id = open_browser_window(ui_view, after, own, saved, at)?;
    // A saved window maximized as it quit, at the frame it un-maximizes to, as the core kept it.
    // Before its UI opens, so that starts at the maximized size.
    if let Some(index) =
        window_frames::saved_at(from.is_none(), own.is_none() && at.is_none(), saved)
        && unsafe { bc::bc_saved_window_maximized(index) }
    {
        maximize(id, ui_view);
    }
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
    unsafe {
        platform::host_window_opened(ui_view, id)
    };
    #[cfg(target_os = "linux")]
    crate::drive::window_made(id, ui_view);
    let from_shell = from.as_ref().and_then(|from| from.shell.upgrade());
    // Its Space, which the core made with the window.
    let own_space = own.map(|own| (unsafe { bc::bc_window_space(id) }, own));
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
            match (&from_shell, own_space) {
                (from, Some((space, Own::Private(profile)))) => {
                    let mut shell =
                        Shell::private_window(engine, space, private_name(profile), profile, cx);
                    if let Some(from) = from {
                        shell.hear_extensions_from(from, cx);
                    }
                    shell
                }
                (from, Some((space, Own::Blank(profile)))) => {
                    Shell::blank_window(engine, space, profile, from.as_ref(), cx)
                }
                (from, Some((space, Own::Little { profile, home }))) => {
                    Shell::little_window(engine, space, home, profile, from.as_ref(), cx)
                }
                (Some(from), None) if saved.is_some() => Shell::seeded_behind(from, engine, cx),
                (Some(from), None) => Shell::seeded_from(from, engine, cx),
                (None, None) => Shell::new(engine, cx),
            }
        });
        window.focus(&shell.focus_handle(cx), cx);
        // As Settings › Features has it, which the core read with the Spaces.
        let little = unsafe { crate::bc::bc_little_windows() };
        shell.update(cx, |shell, cx| shell.set_little_windows(little, cx));
        CX.set(Some(cx.to_async()));
        if let Some(state) = SYNC_STATE.with_borrow_mut(Option::take) {
            settings::sync_changed(state, cx);
        }
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        emoji::keep(cx);
        // In front, as it's about to be key, or a saved one behind the others. One in front is the
        // key shell from now, as macOS reports no key change for the window first in the list, so
        // a window made key before it, such as the one left as another closed, hears too late to
        // take the shortcuts back (#2648).
        if saved.is_none() {
            update_app_later(shell::window_became_key(shell.downgrade()));
        }
        WINDOWS.with_borrow_mut(|windows| {
            windows.insert(
                if saved.is_some() { windows.len() } else { 0 },
                BrowserWindow {
                    id,
                    ui_view,
                    #[cfg(target_os = "linux")]
                    view: ns_view(window),
                    shell: shell.downgrade(),
                    handle: Window::window_handle(window),
                    children: Vec::new(),
                    private: matches!(own, Some(Own::Private(_))),
                    blank: matches!(own, Some(Own::Blank(_))),
                    little: match own {
                        Some(Own::Little { home, .. }) => Some(home),
                        _ => None,
                    },
                    users_space: 0,
                    link: None,
                },
            )
        });
        if from_shell.is_none() {
            // The core read the extensions before there was a shell to take them.
            unsafe { bc::bc_extensions_report() };
        }
        // As at a launch below 20 %, before there was a window to hear it.
        let battery = BATTERY_SAVER.get();
        shell.update(cx, |shell, cx| shell.set_battery_saver(battery, cx));
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
    if let Some(from) = from.as_ref().filter(|_| own.is_none()) {
        // A saved window shows the Space the core put back in it.
        let space = if saved.is_some() {
            unsafe { bc::bc_window_space(id) }
        } else {
            let space = unsafe { bc::bc_window_space(from.id) };
            unsafe { bc::bc_window_show_space_empty(id, space) };
            space
        };
        // Its Space drawn before it's on screen, rather than once the core's report comes.
        window
            .update(cx, |shell, _, cx| shell.set_space(space, cx))
            .ok();
    }
    let first = from.is_none();
    // Once the page frame is set, so the tab shown loads at its size, and before the window is on
    // screen, so the core's reports of its tabs can come before its first frame.
    let restored = first && restore_spaces(id, window, cx);
    // Under Wayland the compositor can answer a window the restore puts back full screen with a
    // configure of no size before its first frame, which Chromium, holding the full screen it asked
    // for (chromium-wayland-state-after-ack.patch), gives no bounds, and the frame it then commits
    // traps. Undone in this task, before any configure is read, it comes back windowed, at its
    // windowed frame, and goes full screen once its UI shows.
    #[cfg(target_os = "linux")]
    let full_screen_later =
        restored && wayland_size.is_some() && unsafe { bc::bc_window_is_full_screen(id) };
    #[cfg(not(target_os = "linux"))]
    let full_screen_later = false;
    if full_screen_later {
        unsafe { bc::bc_window_toggle_full_screen(id) };
        log!("browser: window back windowed until its UI shows");
    }
    if let Some(Own::Little { .. }) = own {
        unsafe { platform::host_set_window_little(ui_view, !cx.reduce_motion()) };
    }
    // A later window comes on screen drawn, as the overlays do: AppKit reports a new window
    // visible, which GPUI on the Mac draws only once it has, about 100 ms after it's ordered in,
    // which the window would spend blank (#2141). The first stays under the launch picture until
    // its own first frame, so it doesn't wait for this one. Through the window alone, as drawing
    // renders the shell, which the WindowHandle's update would hold.
    #[cfg(target_os = "macos")]
    let drawn = !first
        && cx
            .update_window(window.into(), |_, window, cx| window.draw_and_present(cx))
            .is_ok();
    #[cfg(not(target_os = "macos"))]
    let drawn = false;
    match (&from, saved) {
        (Some(from), Some(_)) => unsafe {
            platform::host_show_window_behind(ui_view, from.ui_view)
        },
        _ => unsafe { platform::host_show_window(ui_view) },
    }
    DRAWN_BEFORE_SHOWN.set(drawn.then(Instant::now));
    log_frame(id, ui_view);
    if !first {
        finish_opening(id, ui_view, window, true, saved.is_none(), own, cx);
        return Some(id);
    }
    // check.sh reads this line.
    log!("browser: window on screen at {:.1} ms", unsafe {
        platform::host_since_launch()
    });
    // Making the overlays' windows takes the main thread for a while, so at launch they wait for a
    // turn of their own after the frame that follows the first. That frame draws the sidebar the
    // core has reported by then and takes the launch picture down, so it goes on screen first. A
    // shortcut that wants an overlay before makes them at once (with_overlays). Later windows
    // don't wait, as a page may open in them at once. A window that isn't visible (another
    // desktop, launched hidden) draws no frames, so a timer makes them if the frames haven't.
    let open = move |cx: &mut App| {
        finish_opening(id, ui_view, window, false, false, own, cx);
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
                log!("browser: UI shown at {:.1} ms", unsafe {
                    platform::host_since_launch()
                });
                // Unless the compositor's answer to the full screen undone has put it there.
                if full_screen_later && !unsafe { bc::bc_window_is_full_screen(id) } {
                    unsafe { bc::bc_window_toggle_full_screen(id) };
                }
                cx.update(overlays_now);
                cx.update(agents_core::start);
                // The command bar's settings rows, built in a task of their own once the UI
                // shows.
                cx.update(crate::settings::index_after_launch)
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
                log!("browser: first frame at {:.1} ms", unsafe {
                    platform::host_since_launch()
                });
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
        cx.background_executor()
            .timer(Duration::from_millis(500))
            .await;
        cx.update(finish)
    })
    .detach();
    Some(id)
}

thread_local! {
    /// AppKit has finished launching the app, and so has handed over the links that launched it.
    pub(crate) static FINISHED_LAUNCHING: Cell<bool> = const { Cell::new(false) };
    /// What waits for it: the first window's frame that takes the launch picture down.
    pub(crate) static AFTER_LAUNCH: RefCell<Option<Box<dyn FnOnce(&mut App)>>> = const { RefCell::new(None) };
    /// The overlays' windows at launch, made on the turn after the frame that takes the launch
    /// picture down, as making them holds the main thread a while.
    pub(crate) static OVERLAYS: RefCell<Option<Box<dyn FnOnce(&mut App)>>> = const { RefCell::new(None) };
}

/// Makes the overlays' windows now if they're still to come.
pub(crate) fn overlays_now(cx: &mut App) {
    if let Some(open) = OVERLAYS.take() {
        open(cx);
    }
}

/// Shows an overlay, making the overlays' windows first if a shortcut asks for one before they're
/// made. Then at the end of the update, as making them reads the shell, which is in one.
pub(crate) fn with_overlays(show: impl FnOnce(&mut App) + 'static, cx: &mut App) {
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
pub(crate) fn finish_opening(
    id: bc::bc_id,
    ui_view: *mut c_void,
    window: WindowHandle<Shell>,
    seeded: bool,
    ui_keyboard: bool,
    own: Option<Own>,
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
        find_bar::open(
            id,
            ui_view,
            {
                let shell = shell.downgrade();
                move |cx, right, top, width| {
                    let shell = shell.upgrade()?;
                    Some(shell.read(cx).at_page_corner(right, top, width))
                }
            },
            cx,
        ),
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
    #[cfg(target_os = "linux")]
    crate::active_window::window_opened();
    match own {
        Some(Own::Private(profile)) => {
            log!("browser: private window opened {id} profile {profile}")
        }
        Some(Own::Little { profile, home }) => {
            log!("browser: little window opened {id} profile {profile} home {home}")
        }
        Some(Own::Blank(profile)) => log!("browser: blank window opened {id} profile {profile}"),
        None => {}
    }
    if own.is_some() {
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
        #[cfg(target_os = "macos")]
        browser::text_input::set_window_has_keyboard(|window| {
            platform::host_window_is_key(ns_view(window))
        });
        platform::host_watch_full_screen(full_screen_changed);
        platform::host_watch_window_main(window_main_changed);
        platform::host_watch_swipes(swiped);
        platform::host_watch_option_taps(option_tapped);
        #[cfg(not(target_os = "linux"))]
        platform::host_watch_ui_keyboard(ui_keyboard_left);
        #[cfg(not(target_os = "linux"))]
        platform::host_watch_page_presses(page_pressed);
        platform::host_watch_resigning_active(launch_picture_on_resigning);
        platform::host_watch_active(became_active);
    }
    follow_reduce_motion(cx);
    browser::shell::follow_scroll_bars(cx);
    #[cfg(not(target_os = "linux"))]
    unsafe {
        platform::host_watch_reduce_motion(reduce_motion_changed)
    };
    #[cfg(target_os = "linux")]
    reduce_motion::watch(cx);
    if let Some(bar) = command_bar::open(Rc::new(Bar), cx)
        && let Ok(view) = bar.update(cx, |_, window, _| ns_view(window))
    {
        let radius = command_bar::RADIUS.into();
        elevate(COMMAND_BAR, Level::Modal);
        unsafe { platform::host_add_overlay(COMMAND_BAR, view, radius, command_bar_dismissed) };
    }
    // The rest are made the first time one shows: a hidden overlay's window still holds what its
    // drawing needs, which a user who never opens it shouldn't pay for (#2138).
    use browser::lazy_window::when_asked;
    when_asked::<icon_picker::IconPicker>(
        |cx| {
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
                    platform::host_set_overlay_shape(
                        ICON_PICKER,
                        corners.as_ptr(),
                        corners.len() / 3,
                    );
                    let (x, y) = icon_picker::ARROW_TIP;
                    platform::host_set_overlay_growth(ICON_PICKER, x.into(), y.into());
                }
            }
        },
        cx,
    );
    when_asked::<folder_preview::FolderPreview>(
        |cx| {
            if let Some(preview) = folder_preview::open(Rc::new(FolderPreviewHost), cx)
                && let Ok(view) = preview.update(cx, |_, window, _| ns_view(window))
            {
                let radius = folder_preview::RADIUS.into();
                elevate(FOLDER_PREVIEW, Level::Popover);
                let dismissed = folder_preview_dismissed;
                unsafe { platform::host_add_overlay(FOLDER_PREVIEW, view, radius, dismissed) };
            }
        },
        cx,
    );
    when_asked::<theme_picker::ThemePicker>(
        |cx| {
            if let Some(picker) = theme_picker::open(Rc::new(ThemeHost), cx)
                && let Ok(view) = picker.update(cx, |_, window, _| ns_view(window))
            {
                elevate(THEME_PICKER, Level::Modal);
                unsafe {
                    platform::host_add_overlay(THEME_PICKER, view, 0., theme_picker_dismissed);
                    platform::host_set_overlay_shrink(THEME_PICKER);
                }
            }
        },
        cx,
    );
    let opener: boost_editor::Opener = Rc::new(|cx| {
        let editor = boost_editor::open(Rc::new(BoostEditorHost), cx)?;
        let view = editor.update(cx, |_, window, _| ns_view(window)).ok()?;
        elevate(BOOST_EDITOR, Level::Modal);
        unsafe {
            platform::host_add_overlay(BOOST_EDITOR, view, 0., boost_editor_dismissed);
            platform::host_set_overlay_shrink(BOOST_EDITOR);
        }
        Some(editor)
    });
    boost_editor::open_when_asked(opener, cx);
    when_asked::<dialog::Dialog>(
        |cx| {
            if let Some(dialog) = dialog::open(Rc::new(DialogHost), cx)
                && let Ok(view) = dialog.update(cx, |_, window, _| ns_view(window))
            {
                unsafe { platform::host_add_dialog(view) };
            }
        },
        cx,
    );
    // The window the toast shows over gives its colour and palette.
    let look = |cx: &App| {
        let shell = WINDOWS.with_borrow(|windows| windows.first().map(|w| w.shell.clone()));
        let shell = shell?.upgrade()?;
        Some((shell.read(cx).window_fill(), shell.read(cx).palette()))
    };
    let toast_host = Rc::new(ToastHost {
        view: Cell::new(std::ptr::null_mut()),
    });
    when_asked::<toast::Toast>(
        {
            let toast_host = toast_host.clone();
            move |cx| {
                if let Some(toast) = toast::open(
                    toast_host.clone(),
                    move |cx| look(cx).unwrap_or_default(),
                    cx,
                ) && let Ok(view) = toast.update(cx, |_, window, _| ns_view(window))
                {
                    toast_host.view.set(view);
                    TOAST_VIEW.set(view);
                    unsafe { platform::host_add_toast(view) };
                }
            }
        },
        cx,
    );
    // Auto-archive at launch can come before the toast its note shows in is asked for.
    show_archive_note(cx);
    // A mode's foot bar takes the main button's accent from the window it shows over.
    when_asked::<browser::foot_bar::FootBar>(
        |cx| {
            let palette = |window: u64, cx: &App| {
                let shell = WINDOWS.with_borrow(|windows| {
                    (windows.iter().find(|w| w.id == window)).map(|w| w.shell.clone())
                });
                Some(shell?.upgrade()?.read(cx).palette())
            };
            if let Some(foot_bar) = browser::foot_bar::open(
                Rc::new(FootBarHost),
                move |window, cx| palette(window, cx).unwrap_or_default(),
                cx,
            ) && let Ok(view) = foot_bar.update(cx, |_, window, _| ns_view(window))
            {
                unsafe { platform::host_add_foot_bar(view) };
            }
        },
        cx,
    );
    when_asked::<capture::Capture>(
        move |cx| {
            if let Some(capture) = capture::open(Rc::new(CaptureHost { toast: toast_host }), cx)
                && let Ok(view) = capture.update(cx, |_, window, _| ns_view(window))
            {
                unsafe { platform::host_add_capture(view, capture_dismissed) };
            }
        },
        cx,
    );
    // Each host holds a preview, so a link goes to one before its window is made, which then says
    // whether this one did.
    LINK_PREVIEWS.set(true);
    when_asked::<link_preview::LinkPreview>(
        |cx| {
            let host = Rc::new(LinkPreviewHost {
                holder: Cell::new(std::ptr::null_mut()),
                over: Cell::new(0),
            });
            let mut holder = std::ptr::null_mut();
            if let Some(preview) = link_preview::open(host.clone(), cx)
                && let Ok(view) = preview.update(cx, |_, window, _| ns_view(window))
            {
                holder = unsafe { platform::host_add_link_preview(view, link_preview_dismissed) };
                host.holder.set(holder);
            }
            LINK_PREVIEWS.set(!holder.is_null());
        },
        cx,
    );
    when_asked::<site_controls::SiteControls>(
        |cx| {
            let host = Rc::new(SiteControlsHost {
                view: Cell::new(std::ptr::null_mut()),
            });
            if let Some(panel) = site_controls::open(host.clone(), cx)
                && let Ok(view) = panel.update(cx, |_, window, _| ns_view(window))
            {
                host.view.set(view);
                elevate(SITE_CONTROLS, Level::Popover);
                unsafe { platform::host_add_site_controls(view, site_controls_dismissed) };
            }
        },
        cx,
    );
    when_asked::<menu::ContextMenu>(
        |cx| {
            if let Some(menu) = menu::open(Rc::new(MenuHost), cx)
                && let Ok(view) = menu.update(cx, |_, window, _| ns_view(window))
            {
                unsafe {
                    platform::host_add_menu_window(view, menu_pointer, menu_dismissed, menu_choose)
                };
            }
        },
        cx,
    );
    // Their windows reach this far past the plate, for its shadow.
    let margin = f64::from(browser::elevation::WINDOW_MARGIN);
    when_asked::<tooltip::Tooltip>(
        move |cx| {
            if let Some(tip) = tooltip::open(Rc::new(TooltipHost), cx)
                && let Ok(view) = tip.update(cx, |_, window, _| fading_view(window))
            {
                unsafe { platform::host_add_tooltip(view, margin, tooltip_away) };
            }
        },
        cx,
    );
    // The card's pictures are kept from the first switch, before its window is made.
    unsafe { platform::host_watch_memory_pressure(memory_pressed) };
    when_asked::<hover_card::HoverCard>(
        move |cx| {
            if let Some(card) = hover_card::open(Rc::new(CardHost), cx)
                && let Ok(view) = card.update(cx, |_, window, _| ns_view(window))
            {
                unsafe { platform::host_add_hover_card(view, margin, card_away) };
            }
        },
        cx,
    );
    when_asked::<link_target::LinkTarget>(
        move |cx| {
            if let Some(target) = link_target::open(
                Rc::new(LinkTargetHost),
                move |cx| look(cx).unwrap_or_default(),
                cx,
            ) && let Ok(view) = target.update(cx, |_, window, _| fading_view(window))
            {
                unsafe { platform::host_add_link_target(view, margin, link_pointer) };
            }
        },
        cx,
    );
    when_asked::<zoom_hud::ZoomHud>(
        |cx| {
            if let Some(hud) = zoom_hud::open(Rc::new(HudHost), cx)
                && let Ok(view) = hud.update(cx, |_, window, _| ns_view(window))
            {
                let (right, top) = (zoom_hud::WINDOW_RIGHT, zoom_hud::WINDOW_TOP);
                let margin = f64::from(zoom_hud::MARGIN);
                unsafe { platform::host_add_hud(view, f64::from(right), f64::from(top), margin) };
            }
        },
        cx,
    );
    drop_targets::open(cx);
}

/// Gives an overlay's shadow window its level's shadow, before the overlay is added.
pub(crate) fn elevate(overlay: c_int, level: Level) {
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

// Closing the last window quits, as its close button does in Chrome's last window.
pub(crate) unsafe extern "C" fn window_closed(_: *mut c_void, id: bc::bc_id) {
    let (window, left) = WINDOWS.with_borrow_mut(|windows| {
        let index = windows.iter().position(|window| window.id == id);
        (index.map(|index| windows.remove(index)), windows.len())
    });
    let own = window.as_ref().is_some_and(|window| !window.regular());
    let regular = WINDOWS.with_borrow(|windows| windows.iter().any(BrowserWindow::regular));
    if let Some(window) = window {
        // The user closing the last regular window, which doesn't resign the app, sets where the
        // next launch's window opens. Not in a quit, which closes the windows in no order once the
        // user's Quit has taken the one in front.
        if !own && !regular && !unsafe { bc::bc_quitting() } {
            save_launch_frame(window.ui_view);
        }
        // A sign-in pop-up's size is its page's, not the next Little ilo window's (#1919).
        let popup = crate::little_ilo::popup_window_closed(id);
        if window.little.is_some() && !popup {
            save_little_size(window.ui_view);
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
                front
                    .shell
                    .update(cx, |shell, cx| shell.became_key(cx))
                    .ok();
            }
        });
    }
    log!("browser: window closed {id}");
    // A private or Little ilo window alone doesn't keep the browser: the Spaces' pages closed with
    // the last regular window, which the next launch brings back, so theirs close with them.
    if left == 0 || (!own && !regular) {
        quit_because("the last window closed");
    }
}
