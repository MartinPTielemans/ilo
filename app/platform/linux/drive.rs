//! Test driver for check.sh on Linux, as platform/mac/drive.mm is on the Mac. With BROWSER_DRIVE=1
//! the app reads one command per line on stdin, in drive.mm's language. Its pointer and keys are
//! the X server's own, through XTEST, as a hand's are: the server routes them, with its grabs,
//! crossings, input focus and the window manager in the way (#2721). XTEST goes to whatever window
//! the server picks, so the driver uses it only on a display a check's session started (`xtest`):
//! BROWSER_HEADLESS set, which only check.sh and tools/headless.sh set, and an X server with no
//! screen of a desktop's, Xvfb's one "screen" or tools/headless.sh's Xwayland on a headless sway.
//! Anywhere else it refuses, and its pointer and keys can't do anything.
//!
//! The pointer goes through XTEST alone, a click's modifiers as keys held around it. Some keys
//! still go as X events sent to the window the server would pick, which its client handles as the
//! device's: a held key's repeat, which the server makes only of a key held past its delay; and,
//! until the drive moves them too, a key with Alt, which the window manager may take, and the keys
//! while `flags` holds a modifier, whose modifiers an XKB lock holds.
//!
//! Under Wayland no client can hand another's surfaces input, so there the pointer and keys are
//! sway's virtual devices, in tools/headless.sh's session alone (drive_wayland.rs).
//!
//! The commands are drive.mm's, with what Linux has: click, doubleclick, rightclick, middleclick,
//! sideclick, clickkey, doubleclickkey, rightclickkey, handclick (X11's own), drag, dragkey, press, dragto,
//! release, rightpress, rightdragto, rightrelease, dragover, dragdrop, dragleave, move, warp (Linux's), scroll, wheel, pageswipe, key,
//! keydown, keyup, flags, type, resize, resizekey, place, order, layout, state, title, pasteboard,
//! front, menu, menubar, choose, dismiss, window, window <n> behind, agentspace, agentmayact,
//! agentopen, agentrelease, agentdisconnect, takeback, age, battery, pair and insert, and Windows' stack.
//! desktop on and off, Linux's own, shows the desktop and the windows again as a pager asks,
//! desktop shell maps a desktop window as a desktop shell does and desktop shell off takes it away, and
//! onscreen logs what the X server shows of the driven window and the app's overlays over it.
//! The Mac's modifiers are pressed as the app reads them off the Mac (keys.rs): cmd with Ctrl, ctrl
//! and alt with Alt, and ⌃Tab as Ctrl+Tab; a click's as Chrome reads them, cmd with Ctrl and alt
//! with Alt. flags presses and lets go of those keys, the last key's among them, and the keys after
//! it go with what it holds. key's again, Linux's alone, presses the key again as the app answers
//! the first press in a page, and logs "browser: drive pressed again". insert commits its text to a
//! field of the UI's, not a page's. Linux's own keyonreturn sends a key as key does, the moment
//! the X input focus is back in a browser window from one of the app's own, such as a dialog that
//! closes, as a key typed as it goes reaches the window, before Chromium has run what it posts on
//! reading the focus come back (#1711). scroll turns the wheel a notch for each event,
//! where the Mac's moves the content by points. choose works on ilo's menu and on ilo's dialog,
//! whose buttons it presses by their labels as drive.mm does an alert's, and dismiss on the menu;
//! Linux shows no alerts yet. fullscreen toggles the window's full screen through the core. ime on types as
//! drive.mm's scripted input method does, in the UI's fields alone, as a page's input method is
//! Chromium's, and it can't compose in a page. dragover and dragdrop are XDND's source, as
//! another app's drag is, to the browser window's top-level window or its proxy, and wait for its
//! answer; under Wayland, the data device's, from a client of the driver's own (drive_drags.rs). swipe is Wayland's alone, as X11 sends no touchpad's phases. The others, such as
//! displays, the driver can't do yet, and says so as drive.mm does. pip, which drive.mm doesn't
//! have yet, finds a video's Picture in Picture window, the Mini Player, or a page's, as a call
//! floats in, and logs "browser: pip at <x> <y> <width> <height>" as it is and each time it moves
//! or resizes, then "browser: pip gone", or "browser: pip none" without one. pip doubleclick, pip
//! wheel, and pip press, dragto and release work in it, at points from its top-left corner as the
//! command finds it, or as it was at the press. pip press, dragto and release alone move the
//! display's pointer, through XTest, onto the window only, and the release puts it back once the
//! window manager has let go, logging where the drag left the window. Under Wayland pip is
//! drive_wayland.rs's, from sway's tree, with its virtual pointer and in sway's layout.
//!
//! It logs as drive.mm does: "browser: drive <n>: <command>" as command n arrives, "browser: drive
//! <n>: can't do that" for one it can't, and "browser: drive <n> done, key <window>, focus <view>,
//! active <0|1>, marked <none|start,length>" once Chromium and GPUI have handled its events, with
//! what the UI's field with the keys has marked. The window and view
//! are named as AppKit's classes are on the Mac, so the runs read one vocabulary: key is
//! BrowserCoreNSWindow for a browser window, GPUIPanel for an overlay and NativeWidgetMacNSWindow for
//! a window of Chromium's, such as an extension's popup or a tab's dialog, and focus
//! RenderWidgetHostViewCocoa for a page, AccessKitSubclassOfGPUIView for the UI and GPUIView for an
//! overlay's. Active is whether the input focus is in one of the app's windows.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::{CString, c_int, c_uint, c_void},
    io::BufRead,
    rc::Rc,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use browser::log;
use gpui::{Keystroke, Menu, MenuItem, Modifiers, px};
use x11rb::{
    connection::Connection,
    protocol::{
        Event,
        xproto::{
            self, ChangeWindowAttributesAux, ConfigureWindowAux, ConnectionExt as _, EventMask,
            KeyButMask, KeyPressEvent, StackMode,
        },
        xtest::ConnectionExt as _,
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

use crate::{
    BrowserWindow, CX, MENUS, WINDOWS, bc, floats, link_preview_window, menu_window, overlays,
    panels, platform, popovers, reveal_window, settings_window,
};

unsafe extern "C" {
    // GLib's, from chrome, whose main loop is GLib's.
    fn g_idle_add_full(
        priority: c_int,
        function: extern "C" fn(*mut c_void) -> c_int,
        data: *mut c_void,
        notify: *mut c_void,
    ) -> c_uint;
    fn g_timeout_add(
        interval: c_uint,
        function: extern "C" fn(*mut c_void) -> c_int,
        data: *mut c_void,
    ) -> c_uint;
}

/// GLib's G_PRIORITY_LOW, below its idle priority.
const G_PRIORITY_LOW: c_int = 300;
/// How long, in milliseconds, a command waits for the loop to be free before it runs anyway: past
/// the longest wait seen while windows animate on a loaded machine, about 110, so it only runs
/// early while frames draw without a break.
const IDLE_WAIT: c_uint = 250;

/// Starts reading commands from stdin.
pub fn start() {
    crate::drive_wayland::start();
    std::thread::spawn(|| {
        let mut number = 0;
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if number == 0 {
                crate::drive_wayland::await_first_focus();
            }
            number += 1;
            let n = number;
            idle(Box::new(move || {
                if n == 1 {
                    release_held_modifiers();
                }
                log!("browser: drive {n}: {line}");
                // It's done once the title has come, which it reports itself.
                if let Some(text) = line.strip_prefix("title ").filter(|text| !text.is_empty()) {
                    await_key_title(n, text.to_owned(), 100);
                    return;
                }
                attempt(n, line, None);
            }));
        }
    });
}

/// Runs command `n`, `line`, and again 10 ms on while it asks to be (`again`), until `until`.
fn attempt(n: u64, line: String, until: Option<Instant>) {
    RETRY_UNTIL.set(until);
    let done = run(&line);
    RETRY_UNTIL.set(None);
    if !done {
        if let Some(until) = AGAIN.take() {
            after(10, Box::new(move || attempt(n, line, Some(until))));
            return;
        }
        log!("browser: drive {n}: can't do that");
    }
    if let Some(rest) = LATER.take() {
        rest(Box::new(move || settle(n)));
        return;
    }
    let until = Instant::now() + Duration::from_secs(5);
    match (ACTIVATING.take(), RESIZING.take()) {
        (Some(window), _) => await_focus(n, window, until),
        (_, Some((gpui, width, height))) => await_size(n, gpui, (width, height), until),
        (None, None) => settle(n),
    }
}

/// Has `press` press the key the command is pressing again as the app answers its press in a page
/// (`page_key_answered`), with key's "again": while the app still handles the first press, before
/// Chromium's loop goes on to what it posted, as a quick second press on a busy machine comes. A
/// command whose press no page had drops it once it's done.
pub(super) fn press_again(press: Box<dyn FnOnce() -> Option<()>>) {
    PRESS_AGAIN.set(Some(press));
}

/// The app has answered a page's key, before the page has it (keys.rs's tab_key_before_page): a key
/// to press again (`press_again`) goes now, and is with the X server or the compositor before the
/// app returns.
pub(crate) fn page_key_answered() {
    let Some(press) = PRESS_AGAIN.take() else {
        return;
    };
    let pressed = press().is_some();
    sync();
    crate::drive_wayland::sync();
    // check.sh reads this line.
    log!(
        "browser: drive pressed again{}",
        if pressed { "" } else { ", but couldn't" }
    );
}

/// Has the command running, which can't yet, run again soon, as one naming a window the window
/// manager hasn't listed yet. False once it has been trying for 2 s, when it can't.
pub(super) fn again() -> bool {
    let until = RETRY_UNTIL
        .get()
        .unwrap_or_else(|| Instant::now() + Duration::from_secs(2));
    let trying = Instant::now() < until;
    if trying {
        AGAIN.set(Some(until));
    }
    trying
}

/// Runs `task` on the main thread once nothing more urgent is ready: the X events Chromium and GPUI
/// have to read, and the tasks Chromium has posted. Those run at GLib's idle priority, a few a turn,
/// so a command there could come between them, as a key between the browser window becoming active
/// and the page taking the keyboard back, which reached no page (#1621). Frames drawn without a
/// break, as a shimmer's under software rendering, keep the loop from ever being free, as their
/// sources are above the idle priority, so after IDLE_WAIT the task runs anyway (#1700).
fn idle(task: Box<dyn FnOnce() + Send>) {
    type Slot = Mutex<Option<Box<dyn FnOnce() + Send>>>;
    extern "C" fn run(data: *mut c_void) -> c_int {
        // SAFETY: one of the two references `idle` leaked, each taken back once. The first to run
        // takes the task.
        let slot = unsafe { Arc::from_raw(data as *const Slot) };
        let task = slot.lock().ok().and_then(|mut task| task.take());
        if let Some(task) = task {
            task();
        }
        0
    }
    let slot: Arc<Slot> = Arc::new(Mutex::new(Some(task)));
    let free = Arc::into_raw(slot.clone()) as *mut c_void;
    let late = Arc::into_raw(slot) as *mut c_void;
    unsafe {
        g_idle_add_full(G_PRIORITY_LOW, run, free, std::ptr::null_mut());
        g_timeout_add(IDLE_WAIT, run, late);
    }
}

/// Has the command running go on after it returns, with `rest`, which gets what reports the
/// command done, to call once it has finished, as one that ends at once is.
pub(super) fn go_on(rest: Box<dyn FnOnce(Box<dyn FnOnce()>)>) {
    LATER.set(Some(rest));
}

/// Runs `task` on the main thread `ms` from now, from a task already on it.
pub(super) fn after(ms: u32, task: Box<dyn FnOnce()>) {
    extern "C" fn run(data: *mut c_void) -> c_int {
        // SAFETY: the box `after` leaked, which runs once.
        let task = unsafe { Box::from_raw(data as *mut Box<dyn FnOnce()>) };
        task();
        0
    }
    unsafe { g_timeout_add(ms, run, Box::into_raw(Box::new(task)) as *mut c_void) };
}

/// Reports command `n` done once Chromium and GPUI have handled its events: once the X server has
/// handed them over, and the main loop has read them. Twice, as the server may write the windows'
/// clients' events after its answer to the driver's. GPUI's are handled as it syncs, since xcb
/// may have read them into its queue already, where they don't wake the loop. Then once the
/// windows have drawn what the events changed.
fn settle(n: u64) {
    sync();
    crate::drive_wayland::sync();
    // Until the drag's target has answered, which it does from the loop.
    if drag_waits() || crate::drive_drags::waits() {
        after(10, Box::new(move || settle(n)));
        return;
    }
    // Until the clipboard's owner has answered `pasteboard`, which Chromium does from the loop.
    if paste_waits() {
        after(100, Box::new(move || settle(n)));
        return;
    }
    gpui_linux::sync_display();
    idle(Box::new(move || {
        sync();
        crate::drive_wayland::sync();
        gpui_linux::sync_display();
        drawn(Box::new(move || {
            if let Some(browser) = driven() {
                watch_zoom(browser.window);
            }
            // Past here no press of the command's is still to be answered.
            PRESS_AGAIN.take();
            let (key, focus, active) = keyboard();
            let marked = Field::with_keys().and_then(Field::marked);
            let marked = marked.map_or("none".into(), |at| format!("{},{}", at.start, at.len()));
            log!(
                "browser: drive {n} done, key {key}, focus {focus}, active {}, marked {marked}",
                active as u8
            );
        }));
    }));
}

/// Runs `then` once each browser window has drawn its next frame, as AppKit has by the time the
/// Mac's driver reports: GPUI hit-tests the frame it drew last, so a click right after a command
/// that changed the layout, such as a double-click that opens a field, would hit what was there
/// before. A window that draws nothing, as a minimized one, is given up on after a few frames.
fn drawn(then: Box<dyn FnOnce()>) {
    let then = Rc::new(Cell::new(Some(then)));
    // After the frame's draw, which follows its next-frame callbacks.
    let finish = move || {
        if let Some(then) = then.take() {
            after(0, then);
        }
    };
    let handles: Vec<_> = WINDOWS.with_borrow(|windows| windows.iter().map(|w| w.handle).collect());
    let left = Rc::new(Cell::new(handles.len()));
    if let Some(cx) = CX.with_borrow(Clone::clone) {
        for handle in handles {
            let (waiting, finish) = (left.clone(), finish.clone());
            let asked = cx.update(|cx| {
                handle.update(cx, |_, window, _| {
                    window.on_next_frame(move |_, _| {
                        waiting.set(waiting.get() - 1);
                        if waiting.get() == 0 {
                            finish();
                        }
                    })
                })
            });
            if asked.is_err() {
                left.set(left.get() - 1);
            }
        }
    }
    match left.get() {
        0 => finish(),
        _ => after(100, Box::new(finish)),
    }
}

thread_local! {
    /// The driver's own connection to the X server, with its root window.
    static X: RefCell<Option<Rc<(RustConnection, xproto::Window)>>> = const { RefCell::new(None) };
    /// Each browser window the app has made, in order, by its UI's X window (`host_ui_view`'s
    /// order on the Mac): the window command's numbers. Numbered as it's made, so one that closes
    /// before a command reaches it, as a pop-up that closes itself, still counts, as on the Mac.
    static MADE: RefCell<Vec<(bc::bc_id, xproto::Window)>> = const { RefCell::new(Vec::new()) };
    /// The window command's, from 0.
    static DRIVEN: Cell<usize> = const { Cell::new(0) };
    /// The window the window command asked the window manager to give the keyboard, which it's
    /// done once it has.
    static ACTIVATING: Cell<Option<xproto::Window>> = const { Cell::new(None) };
    /// The UI's view of the browser window the resize command asked a size of, in points, which
    /// it's done once the UI has.
    static RESIZING: Cell<Option<(usize, f64, f64)>> = const { Cell::new(None) };
    /// The rest of a command that goes on after it returns, as a swipe that rests before the
    /// fingers lift (drive_wayland.rs), which reports it done (go_on).
    static LATER: Cell<Option<Box<dyn FnOnce(Box<dyn FnOnce()>)>>> = const { Cell::new(None) };
    /// Until when the command running may run again, once it has asked to (`again`).
    static RETRY_UNTIL: Cell<Option<Instant>> = const { Cell::new(None) };
    /// That the command running asked to run again, and until when.
    static AGAIN: Cell<Option<Instant>> = const { Cell::new(None) };
    /// A key's second press, which the app's answer to the first page key presses (`press_again`).
    static PRESS_AGAIN: Cell<Option<Box<dyn FnOnce() -> Option<()>>>> = const { Cell::new(None) };
    /// The window a press went to, which gets the drag's events until the release, as the X
    /// server's grab of the pointer does.
    static PRESSED: Cell<Option<Target>> = const { Cell::new(None) };
    /// The Mini Player's press through XTest, till its release (hold_mini_player).
    static PIP_HELD: Cell<Option<HeldPip>> = const { Cell::new(None) };
    /// The window the pointer is in, which hears it leave.
    static HOVERED: Cell<Option<Target>> = const { Cell::new(None) };
    /// The modifiers the flags command holds, in an X state, which the keys go with until it lets
    /// them go.
    static FLAGS: Cell<u16> = const { Cell::new(0) };
    /// The modifiers the last key went with.
    static KEY_FLAGS: Cell<u16> = const { Cell::new(0) };
    /// The modifiers the X server's keyboard has locked, for `hold_modifiers`.
    static LOCKED: Cell<u16> = const { Cell::new(0) };
    /// The modifier keys the driver holds down through XTEST.
    static XTEST_MODIFIERS: Cell<u16> = const { Cell::new(0) };
    /// The other keys the driver holds down through XTEST, whose next press is a repeat.
    static XTEST_KEYS: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    /// Whether the display is one XTEST may drive (`xtest`), once known.
    static XTEST: Cell<Option<bool>> = const { Cell::new(None) };
    /// The last press, a click's or a drag's, in any of the app's windows: its time, place and
    /// button.
    static LAST_CLICK: Cell<Option<(std::time::Instant, Target, u8)>> = const { Cell::new(None) };
    /// The keysyms on each key, from the server's first keycode, and how many each key has.
    static KEYMAP: RefCell<Option<(u8, usize, Vec<u32>)>> = const { RefCell::new(None) };
    /// Whether the scripted input method types (`ime_key`), and what it composes, if anything.
    static IME_ON: Cell<bool> = const { Cell::new(false) };
    static COMPOSITION: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn server() -> Option<Rc<(RustConnection, xproto::Window)>> {
    // Under Wayland the views are pointers, which as X ids could name another client's windows.
    let window = WINDOWS.with_borrow(|windows| windows.first().map(|w| w.id))?;
    let (mut width, mut height) = (0., 0.);
    let wayland = unsafe { bc::bc_window_wayland_surface(window, &mut width, &mut height) };
    wayland.is_null().then_some(())?;
    X.with_borrow_mut(|x| {
        if x.is_none() {
            let (connection, screen) = x11rb::connect(None).ok()?;
            let root = &connection.setup().roots[screen];
            // The display's pointer waits in the screen's corner, out of the windows, until the
            // first command moves it.
            let corner = (
                root.width_in_pixels as i16 - 1,
                root.height_in_pixels as i16 - 1,
            );
            let root = root.root;
            _ = connection.warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, corner.0, corner.1);
            _ = connection.flush();
            *x = Some(Rc::new((connection, root)));
        }
        x.clone()
    })
}

/// The views of the app's windows that show over a browser window: the overlays, the Site Controls
/// and Capture, the find bar and the offer to save a password, then ilo's menu, and the toast.
pub(super) fn overlay_views() -> Vec<*mut c_void> {
    let mut views = overlays::shown();
    views.extend(popovers::shown());
    views.extend(panels::shown());
    views.extend(menu_window::shown());
    views.extend(reveal_window::shown());
    views.extend(floats::shown());
    views.extend(link_preview_window::shown());
    views
}

/// Waits for the X server to have handled what the driver sent, and drops the errors it answered
/// with, such as for a window that has closed since.
fn sync() {
    let Some(x) = server() else { return };
    let (connection, _) = &*x;
    _ = connection.get_input_focus().map(|cookie| cookie.reply());
    while let Ok(Some(event)) = connection.poll_for_event() {
        match event {
            Event::SelectionRequest(request) => serve_drag(&request),
            Event::SelectionNotify(notify) => pasted(&notify),
            Event::ClientMessage(message) if message.format == 32 => drag_answered(&message),
            _ => {}
        }
    }
}

/// A browser window, as the X server has it.
#[derive(Clone, Copy)]
struct Browser {
    id: bc::bc_id,
    /// Chromium's window.
    window: xproto::Window,
    /// The app's UI's window in it, the view the core holds (`host_ui_view`), and GPUI's window,
    /// which fills it.
    ui: xproto::Window,
    gpui: xproto::Window,
}

/// Numbers the browser window the app just made, for the window command.
pub(crate) fn window_made(id: bc::bc_id, ui_view: *mut std::ffi::c_void) {
    match crate::drive_wayland::on() {
        true => crate::drive_wayland::window_made(id),
        false => MADE.with_borrow_mut(|made| made.push((id, ui_view as usize as u32))),
    }
}

/// The browser window the app made `index`th, from 0, while it's open.
fn browser(index: usize) -> Option<Browser> {
    let (id, ui) = MADE.with_borrow(|made| made.get(index).copied())?;
    let open = WINDOWS.with_borrow(|windows| {
        (windows.iter()).any(|w: &BrowserWindow| w.id == id && w.ui_view as usize as u32 == ui)
    });
    open.then_some(())?;
    let x = server()?;
    let (connection, _) = &*x;
    // Under Wayland the view is a surface, which the X server knows nothing of.
    let tree = connection.query_tree(ui).ok()?.reply().ok()?;
    Some(Browser {
        id,
        window: tree.parent,
        ui,
        gpui: *tree.children.first()?,
    })
}

fn driven() -> Option<Browser> {
    browser(DRIVEN.get())
}

/// The driven browser window's id, under Wayland too.
fn driven_id() -> Option<bc::bc_id> {
    match crate::drive_wayland::on() {
        true => crate::drive_wayland::driven_id(),
        false => driven().map(|browser| browser.id),
    }
}

/// Whether `window` is mapped, with every window it's in: the server sends no key to one that isn't.
fn viewable(connection: &RustConnection, window: xproto::Window) -> bool {
    connection
        .get_window_attributes(window)
        .ok()
        .and_then(|c| c.reply().ok())
        .is_some_and(|attributes| attributes.map_state == xproto::MapState::VIEWABLE)
}

/// Whether `window` is `ancestor` or inside it.
fn within(
    connection: &RustConnection,
    mut window: xproto::Window,
    ancestor: xproto::Window,
) -> bool {
    while window != x11rb::NONE {
        if window == ancestor {
            return true;
        }
        let Some(tree) = connection
            .query_tree(window)
            .ok()
            .and_then(|c| c.reply().ok())
        else {
            return false;
        };
        if tree.parent == tree.root {
            return false;
        }
        window = tree.parent;
    }
    false
}

/// The overlay, Settings or the browser window the X server's input focus is in, if it's the
/// app's, with the focus, and Settings' window.
enum Keyboard {
    Overlay(xproto::Window),
    Settings(xproto::Window, xproto::Window),
    Browser(Browser, xproto::Window),
}

fn keyboard_window() -> Option<Keyboard> {
    let x = server()?;
    let (connection, _) = &*x;
    let focus = connection.get_input_focus().ok()?.reply().ok()?.focus;
    for view in overlay_views() {
        if within(connection, focus, view as usize as u32) {
            return Some(Keyboard::Overlay(focus));
        }
    }
    if let Some(view) = settings_window::shown()
        && within(connection, focus, view as usize as u32)
    {
        return Some(Keyboard::Settings(view as usize as u32, focus));
    }
    let count = MADE.with_borrow(Vec::len).max(1);
    (0..count + WINDOWS.with_borrow(Vec::len))
        .filter_map(browser)
        .find(|browser| within(connection, focus, browser.window))
        .map(|browser| Keyboard::Browser(browser, focus))
}

/// Commits `text` to the field of the UI or overlay with the keyboard, as an input method does, or
/// the Mac's character palette: as a keystroke that types it, with no key, which GPUI hands the
/// field as it would a commit. A page with the keyboard gets its characters as plain keys, as
/// Windows' driver posts them, since Chromium's input method is out of the driver's reach.
fn insert(text: &str) -> Option<()> {
    let x = server()?;
    let (connection, _) = &*x;
    let focus = connection.get_input_focus().ok()?.reply().ok()?.focus;
    let cx = CX.with_borrow(Clone::clone)?;
    let typed = Keystroke {
        modifiers: Modifiers::default(),
        key: String::new(),
        key_char: Some(text.to_string()),
    };
    let in_ui = cx.update(|cx| {
        let handle = cx.windows().into_iter().find(|handle| {
            (handle.update(cx, |_, window, _| crate::ns_view(window) as usize as u32))
                .is_ok_and(|view| within(connection, focus, view))
        })?;
        let typed = handle.update(cx, |_, window, cx| window.dispatch_keystroke(typed, cx));
        Some(typed.is_ok_and(|typed| typed))
    });
    match in_ui {
        Some(typed) => typed.then_some(()),
        None => {
            matches!(keyboard_window(), Some(Keyboard::Browser(..))).then_some(())?;
            let kinds = [xproto::KEY_PRESS_EVENT, xproto::KEY_RELEASE_EVENT];
            text.chars()
                .try_for_each(|c| key_events(&c.to_string(), 0, &kinds))
        }
    }
}

/// The done line's key window, focus and whether the app is active.
fn keyboard() -> (&'static str, &'static str, bool) {
    if crate::drive_wayland::on() {
        return crate::drive_wayland::keyboard();
    }
    match keyboard_window() {
        Some(Keyboard::Overlay(_)) => ("GPUIPanel", "GPUIView", true),
        Some(Keyboard::Settings(..)) => ("GPUIWindow", "GPUIView", true),
        Some(Keyboard::Browser(browser, _)) => {
            if unsafe { bc::bc_window_keyboard_in_child(browser.id) } {
                return ("NativeWidgetMacNSWindow", "RenderWidgetHostViewCocoa", true);
            }
            match unsafe { bc::bc_window_keyboard_is_ui(browser.id) } {
                true => ("BrowserCoreNSWindow", "AccessKitSubclassOfGPUIView", true),
                false => ("BrowserCoreNSWindow", "RenderWidgetHostViewCocoa", true),
            }
        }
        None => ("none", "none", false),
    }
}

/// Where a pointer event goes: a window of the app's, and the point in its pixels and the root's.
#[derive(Clone, Copy, PartialEq)]
struct Target {
    window: xproto::Window,
    x: i16,
    y: i16,
    root_x: i16,
    root_y: i16,
}

fn scale() -> f64 {
    unsafe { bc::bc_display_scale() }
}

/// The point (`x`, `y`), in points from `window`'s top-left corner, in `window`.
fn target_in(window: xproto::Window, x: f64, y: f64) -> Option<Target> {
    let connection = server()?;
    let (connection, root) = &*connection;
    let (x, y) = ((x * scale()).round() as i16, (y * scale()).round() as i16);
    let at = connection
        .translate_coordinates(window, *root, x, y)
        .ok()?
        .reply()
        .ok()?;
    Some(Target {
        window,
        x,
        y,
        root_x: at.dst_x,
        root_y: at.dst_y,
    })
}

/// The window of the app's that the pointer at (`x`, `y`) in the driven browser window is over
/// (`target_at_in`).
fn target_at(x: f64, y: f64) -> Option<Target> {
    target_at_in(driven()?, x, y)
}

/// The window of the app's that the pointer at (`x`, `y`) in `browser` is over, as the X server
/// picks it: an overlay over the window, or a panel in it, then GPUI's window, but where its shape
/// leaves holes for the pages, which are Chromium's window's.
fn target_at_in(browser: Browser, x: f64, y: f64) -> Option<Target> {
    let at = target_in(browser.window, x, y)?;
    let connection = server()?;
    let (connection, root) = &*connection;
    let top = connection
        .translate_coordinates(*root, *root, at.root_x, at.root_y)
        .ok()?
        .reply()
        .ok()?;
    let child = (connection.translate_coordinates(browser.window, browser.window, at.x, at.y))
        .ok()?
        .reply()
        .ok()?
        .child;
    // An overlay is a child of the root, and a panel, such as the find bar or the offer to save a
    // password, one of the browser window's, over its page.
    if let Some(view) = overlay_views()
        .into_iter()
        .find(|&view| [top.child, child].contains(&(view as usize as u32)))
    {
        let view = view as usize as u32;
        let inside = connection
            .translate_coordinates(*root, view, at.root_x, at.root_y)
            .ok()?
            .reply()
            .ok()?;
        return Some(Target {
            window: view,
            x: inside.dst_x,
            y: inside.dst_y,
            ..at
        });
    }
    // The toast lets the pointer through but on its plate, which a click there lands on.
    if let Some((view, plate)) = floats::toast_plate()
        && top.child == view as usize as u32
    {
        let view = view as usize as u32;
        let inside = connection
            .translate_coordinates(*root, view, at.root_x, at.root_y)
            .ok()?
            .reply()
            .ok()?;
        let point = |each: i16| px((f64::from(each) / scale()) as f32);
        if plate.contains(&gpui::point(point(inside.dst_x), point(inside.dst_y))) {
            return Some(Target {
                window: view,
                x: inside.dst_x,
                y: inside.dst_y,
                ..at
            });
        }
    }
    Some(match child == browser.ui {
        true => Target {
            window: browser.gpui,
            ..at
        },
        false => at,
    })
}

/// How far the Mac's close button is in from its window's top-left corner, at most, in points.
pub(super) const CLOSE_CORNER: f64 = 26.;

/// Asks `window` to close, as the window manager does for its close button.
fn close(window: xproto::Window) -> Option<()> {
    let x = server()?;
    let (connection, _) = &*x;
    let [protocols, delete] = ["WM_PROTOCOLS", "WM_DELETE_WINDOW"].map(|name| {
        let atom = connection.intern_atom(false, name.as_bytes()).ok()?;
        Some(atom.reply().ok()?.atom)
    });
    let data = [delete?, x11rb::CURRENT_TIME, 0, 0, 0];
    let message = xproto::ClientMessageEvent::new(32, window, protocols?, data);
    connection
        .send_event(false, window, EventMask::NO_EVENT, message)
        .ok()?;
    connection.flush().ok()
}

/// The point (`x`, `y`) in points from the key window's top-left corner, such as the command
/// bar's or a Little ilo window's that has the keyboard though another is driven, as the Mac's
/// driver clicks in its key window, or else the driven browser window's.
fn target_in_key(x: f64, y: f64) -> Option<Target> {
    match keyboard_window() {
        Some(Keyboard::Overlay(window) | Keyboard::Settings(window, _)) => target_in(window, x, y),
        Some(Keyboard::Browser(browser, _)) => target_at_in(browser, x, y),
        None => target_at(x, y),
    }
}

/// The state's bit for `button` held: X has one for each of the first five, the wheel's among
/// them, and none for the others.
fn button_mask(button: u8) -> u16 {
    match button {
        1..=5 => 1 << (7 + button),
        _ => 0,
    }
}

/// Moves the pointer to `to` through XTEST, with the server's crossings. Not when it's there
/// already, as a hand sends no move then: Blink takes a move while a button is down, even to the
/// same point, for a drag, and the release then clicks nothing (#2717, #2736).
fn hover(to: Target) -> Option<()> {
    HOVERED.set(Some(to));
    let x = server()?;
    let (connection, root) = &*x;
    let at = connection.query_pointer(*root).ok()?.reply().ok()?;
    if (at.root_x, at.root_y) == (to.root_x, to.root_y) {
        return Some(());
    }
    xtest_event(xproto::MOTION_NOTIFY_EVENT, 0, to.root_x, to.root_y)
}

/// A press of `button`, 1 the left, 3 the right and 4 to 7 the wheel's, or its release, at `at`,
/// through XTEST, with `state` the modifiers and the buttons held before it. The modifiers' keys
/// go down before the press and up after the release.
fn button(at: Target, button: u8, press: bool, state: u16) -> Option<()> {
    if press {
        // A hand lifts the ⌘ of its last chord before it clicks, and holds the click's own: the
        // Control a key left down would make the press a Ctrl-click.
        if XTEST_MODIFIERS.get() != 0 {
            KEY_FLAGS.set(0);
        }
        xtest_modifiers(state & MODIFIERS)?;
    }
    hover(at)?;
    let kind = match press {
        true => xproto::BUTTON_PRESS_EVENT,
        false => xproto::BUTTON_RELEASE_EVENT,
    };
    xtest_event(kind, button, at.root_x, at.root_y)?;
    match press {
        true => Some(()),
        false => xtest_modifiers(0),
    }
}

/// Moves the pointer to `at` through XTEST, which holds the buttons itself. While one is held, the
/// modifiers' keys `state` names are held with it, as a drag's ⌥ that drops a copy comes during it.
fn motion(at: Target, state: u16) -> Option<()> {
    if state & BUTTONS != 0 {
        xtest_modifiers(state & MODIFIERS)?;
    }
    hover(at)
}

/// The five buttons X has a state's bit for.
const BUTTONS: u16 = 0x1f00;

/// Control, Shift and Alt, in an X state.
const MODIFIERS: u16 = 1 << 2 | 1 | 1 << 3;

/// Whether XTEST may drive this display: only one a check's session started, as its input goes to
/// whatever window the X server picks, and on someone's own desktop that would be their other
/// windows. Asked once, and said once in the log. Under Xvfb, as check.sh and CI start it, with
/// openbox or KWin, BROWSER_HEADLESS is "xvfb" and the server's one RandR output is Xvfb's
/// "screen", where a desktop's are named for its connectors. On tools/headless.sh's Xwayland it's
/// the session's socket, whose sway's outputs are all headless (drive_wayland::headless_sway), and
/// the server's outputs are Xwayland's.
fn xtest() -> Option<()> {
    if XTEST.get().is_none() {
        let x = server()?;
        let (connection, root) = &*x;
        let answer = xtest_session(connection, *root);
        match &answer {
            Ok(session) => log!("browser: drive input through XTEST, on {session}"),
            Err(why) => log!("browser: drive refuses XTEST input here: {why}"),
        }
        XTEST.set(Some(answer.is_ok()));
    }
    XTEST.get()?.then_some(())
}

fn xtest_session(
    connection: &RustConnection,
    root: xproto::Window,
) -> Result<&'static str, String> {
    let headless = std::env::var("BROWSER_HEADLESS").unwrap_or_default();
    let outputs = output_names(connection, root).unwrap_or_default();
    xtest_verdict(&headless, &outputs, || {
        crate::drive_wayland::headless_sway().map(drop)
    })
}

/// `xtest`'s answer for the session `headless` names (BROWSER_HEADLESS), on an X server whose
/// RandR outputs are `outputs`, with `sway` asked whether the sway at SWAYSOCK is headless.
fn xtest_verdict(
    headless: &str,
    outputs: &[String],
    sway: impl FnOnce() -> Result<(), String>,
) -> Result<&'static str, String> {
    if headless.is_empty() {
        return Err("BROWSER_HEADLESS isn't set, which a check's session sets".into());
    }
    let all = |test: &dyn Fn(&str) -> bool| !outputs.is_empty() && outputs.iter().all(|o| test(o));
    if headless == "xvfb" {
        return match all(&|output| output == "screen") {
            true => Ok("Xvfb"),
            false => Err(format!(
                "the X server's outputs are {outputs:?}, not Xvfb's \"screen\""
            )),
        };
    }
    sway()?;
    // Xwayland names its outputs for the compositor's, or XWAYLAND<n> where it can't.
    match all(&|output| output.starts_with("HEADLESS-") || output.starts_with("XWAYLAND")) {
        true => Ok("tools/headless.sh's Xwayland"),
        false => Err(format!(
            "the X server's outputs are {outputs:?}, not a headless sway's"
        )),
    }
}

/// The names of the X server's RandR outputs.
fn output_names(connection: &RustConnection, root: xproto::Window) -> Option<Vec<String>> {
    use x11rb::protocol::randr::ConnectionExt as _;
    let resources = (connection
        .randr_get_screen_resources_current(root)
        .ok()?
        .reply())
    .ok()?;
    (resources.outputs.iter())
        .map(|&output| {
            let info = connection.randr_get_output_info(output, resources.config_timestamp);
            Some(String::from_utf8_lossy(&info.ok()?.reply().ok()?.name).into_owned())
        })
        .collect()
}

/// One XTEST event at the root's `x`, `y`, waited on until the server has handled it, on a
/// display `xtest` allows.
fn xtest_event(kind: u8, detail: u8, x: i16, y: i16) -> Option<()> {
    xtest()?;
    let server = server()?;
    let (connection, root) = &*server;
    let delay = xtest_delay(kind);
    (connection.xtest_fake_input(kind, detail, delay, *root, x, y, 0)).ok()?;
    connection.get_input_focus().ok()?.reply().ok().map(drop)
}

/// The milliseconds the server waits before an XTEST event of `kind`, XTEST's time being a delay.
/// A press waits one, as a hand's two presses never share a millisecond: Chromium takes a press
/// stamped as its last for that one handled again, and counts no double click
/// (MouseEvent::GetRepeatCount).
fn xtest_delay(kind: u8) -> u32 {
    match kind {
        xproto::BUTTON_PRESS_EVENT => 1,
        _ => x11rb::CURRENT_TIME,
    }
}

/// The left Control, Shift and Alt keys, by their masks.
const MODIFIER_KEYS: [(u16, u32); 3] = [(1 << 2, 0xffe3), (1, 0xffe1), (1 << 3, 0xffe9)];

/// Presses and lets go of the left modifier keys through XTEST until the server holds down `want`'s
/// Control, Shift and Alt, and no others of them.
fn xtest_modifiers(want: u16) -> Option<()> {
    for (mask, keysym) in MODIFIER_KEYS {
        let kind = match (XTEST_MODIFIERS.get() & mask != 0, want & mask != 0) {
            (false, true) => xproto::KEY_PRESS_EVENT,
            (true, false) => xproto::KEY_RELEASE_EVENT,
            _ => continue,
        };
        xtest_event(kind, keycode(keysym)?.0, 0, 0)?;
        XTEST_MODIFIERS.set(XTEST_MODIFIERS.get() ^ mask);
    }
    Some(())
}

/// A click of `which` (1 the left, 3 the right, 4 to 7 the wheel's) at `at`, the second of a
/// double click with `again`.
fn click(at: Target, which: u8, state: u16, again: bool) -> Option<()> {
    first_press(at, which, again);
    button(at, which, true, state)?;
    button(at, which, false, state | button_mask(which))
}

/// Before a press of `which` at `at`: unless it's `again`, the second of a double click, waits
/// until neither GPUI nor Chromium would count it with the last press, as the driver presses
/// faster than a hand does. A drag's press counts too, as GPUI counts it.
fn first_press(at: Target, which: u8, again: bool) {
    if which <= 3
        && !again
        && let Some((then, last, last_which)) = LAST_CLICK.get()
        && last_which == which
        && (last.x - at.x).abs() <= 5 * scale().ceil() as i16
        && (last.y - at.y).abs() <= 5 * scale().ceil() as i16
    {
        // Chromium's kDoubleClickTimeMs, 500, with a margin; GPUI's interval is shorter. Chromium
        // counts clicks across its windows, by their places in each, so a click in the page at
        // the place of one in another window just before is its second, and selects a word.
        std::thread::sleep(std::time::Duration::from_millis(550).saturating_sub(then.elapsed()));
    }
    if which <= 3 {
        LAST_CLICK.set(Some((std::time::Instant::now(), at, which)));
    }
}

/// The Mac's modifiers named in `words`, cmd, shift, alt and ctrl, as an X state holds them on
/// the keys that press them off the Mac: ⌘ with Ctrl, and ⌃ and ⌥ with Alt (docs/ux-spec.md ›
/// Keyboard), so ⌥⌘↓ is Ctrl+Alt+↓ and ⌥↩ is Alt+Enter.
pub(super) fn modifiers<'a>(words: impl Iterator<Item = &'a str>) -> u16 {
    let (ctrl, alt) = (u16::from(KeyButMask::CONTROL), u16::from(KeyButMask::MOD1));
    words.fold(0, |state, word| {
        state
            | match word {
                "shift" => u16::from(KeyButMask::SHIFT),
                "cmd" => ctrl,
                "ctrl" | "alt" => alt,
                _ => 0,
            }
    })
}

/// The Mac's modifiers named in `words` for a click, as Chrome reads a click's off the Mac: ⌘ with
/// Ctrl and ⌥ with Alt. Its ⌥⌘-click is Ctrl+Alt there, and Ctrl+Alt+Shift, as keys press ⌥⇧, is
/// a foreground tab's. ⌃-click, the Mac's right-click, has none.
pub(super) fn click_modifiers<'a>(words: impl Iterator<Item = &'a str>) -> u16 {
    words.fold(0, |state, word| {
        state
            | match word {
                "shift" => u16::from(KeyButMask::SHIFT),
                "cmd" => u16::from(KeyButMask::CONTROL),
                "alt" => u16::from(KeyButMask::MOD1),
                _ => 0,
            }
    })
}

/// The keysym of drive.mm's key `key`: a printable ASCII character, or a named key.
pub(super) fn keysym(key: &str) -> Option<u32> {
    Some(match key {
        "return" => 0xff0d,
        "escape" => 0xff1b,
        "tab" => 0xff09,
        "space" => 0x20,
        "backspace" => 0xff08,
        "delete" => 0xffff,
        "home" => 0xff50,
        "pageup" => 0xff55,
        "pagedown" => 0xff56,
        "up" => 0xff52,
        "down" => 0xff54,
        "left" => 0xff51,
        "right" => 0xff53,
        "f6" => 0xffc3,
        "f10" => 0xffc7,
        "f12" => 0xffc9,
        _ => {
            let mut characters = key.chars();
            match (characters.next(), characters.next()) {
                (Some(c), None) if c.is_ascii_graphic() || c == ' ' => c as u32,
                _ => return None,
            }
        }
    })
}

/// The key that types `keysym` on the server's keyboard, and whether with shift.
fn keycode(keysym: u32) -> Option<(u8, bool)> {
    KEYMAP.with_borrow_mut(|keymap| {
        if keymap.is_none() {
            let x = server()?;
            let (connection, _) = &*x;
            let (first, last) = (
                connection.setup().min_keycode,
                connection.setup().max_keycode,
            );
            let map = connection
                .get_keyboard_mapping(first, last - first + 1)
                .ok()?
                .reply()
                .ok()?;
            *keymap = Some((first, usize::from(map.keysyms_per_keycode), map.keysyms));
        }
        let (first, per, keysyms) = keymap.as_ref()?;
        (0..keysyms.len() / per).find_map(|key| {
            let level = (0..(*per).min(2)).find(|&level| keysyms[key * per + level] == keysym)?;
            Some((*first + key as u8, level == 1))
        })
    })
}

/// Presses and releases `key` with the modifiers `state` in the window with the keyboard, which is
/// the driven browser window's unless the input focus is in one of the app's.
fn key(key: &str, state: u16) -> Option<()> {
    if ime_key(key, state)? {
        return Some(());
    }
    key_events(
        key,
        state,
        &[xproto::KEY_PRESS_EVENT, xproto::KEY_RELEASE_EVENT],
    )
}

/// The window with the input focus.
fn input_focus() -> Option<xproto::Window> {
    let x = server()?;
    Some(x.0.get_input_focus().ok()?.reply().ok()?.focus)
}

/// The UI's field the scripted input method composes in: GPUI's in the X window with the input
/// focus, or under Wayland in the browser window's surface while its UI or an overlay over it has
/// the keys, which the compositor gives the surface.
#[derive(Clone, Copy)]
enum Field {
    X(xproto::Window),
    Wayland(*mut c_void),
}

impl Field {
    fn with_keys() -> Option<Field> {
        match crate::drive_wayland::on() {
            true => crate::drive_wayland::ime_surface().map(Field::Wayland),
            false => input_focus().map(Field::X),
        }
    }

    fn compose(self, text: &str, selected: Option<std::ops::Range<usize>>) -> bool {
        match self {
            Field::X(window) => gpui_linux::drive_compose(window, text, selected),
            Field::Wayland(surface) => gpui_linux::drive_compose_in(surface, text, selected),
        }
    }

    fn commit(self, text: &str) -> bool {
        match self {
            Field::X(window) => gpui_linux::drive_commit(window, text),
            Field::Wayland(surface) => gpui_linux::drive_commit_in(surface, text),
        }
    }

    fn marked(self) -> Option<std::ops::Range<usize>> {
        match self {
            Field::X(window) => gpui_linux::marked_range(window),
            Field::Wayland(surface) => gpui_linux::marked_range_in(surface),
        }
    }
}

/// Handles `key` as drive.mm's scripted input method does, with the calls a Japanese one makes in
/// the UI's field with the keys: letters compose, space converts them all, return commits and
/// escape cancels. Returns false for a key it leaves to the window, as one with a shortcut's
/// modifiers, and None for one it would compose in a page, whose input method is Chromium's,
/// which the driver can't type through: as plain keys it would pass for a composition.
pub(super) fn ime_key(key: &str, state: u16) -> Option<bool> {
    let shortcut = u16::from(KeyButMask::CONTROL | KeyButMask::MOD1);
    let letter = key.len() == 1 && key.bytes().all(|b| b.is_ascii_lowercase());
    let composing = COMPOSITION.with_borrow(Option::is_some);
    if !IME_ON.get() || (state | FLAGS.get()) & shortcut != 0 || !(composing || letter) {
        return Some(false);
    }
    let field = Field::with_keys()?;
    let composition = COMPOSITION.take();
    let mark = |text: &str, selected: std::ops::Range<usize>| {
        let handled = field.compose(text, Some(selected));
        COMPOSITION.set(Some(text.to_string()));
        handled
    };
    let handled = match key {
        "return" => field.commit(composition.as_deref().unwrap_or_default()),
        "escape" => field.compose("", None),
        // As a real one does, the conversion is selected whole.
        "space" => mark("日本語", 0..3),
        _ if letter => {
            let text = composition.clone().unwrap_or_default() + key;
            let end = text.len();
            mark(&text, end..end)
        }
        _ => {
            COMPOSITION.set(composition.clone());
            true
        }
    };
    if !handled {
        COMPOSITION.set(composition);
        return None;
    }
    Some(true)
}

/// Sends `key`'s events of `kinds` as `key` does. A held key's repeats are presses without
/// releases between them, as the X server sends them while it auto-repeats.
fn key_events(key: &str, mut state: u16, kinds: &[u8]) -> Option<()> {
    let (code, shift) = keycode(keysym(key)?)?;
    state |= FLAGS.get();
    let modifiers = KeyButMask::CONTROL | KeyButMask::MOD1 | KeyButMask::SHIFT;
    KEY_FLAGS.set(state & u16::from(modifiers));
    hold_modifiers();
    if shift {
        state |= u16::from(KeyButMask::SHIFT);
    }
    code_events(code, state, kinds)
}

/// Has the X server's keyboard hold Control while the driver does, after `flags` or a key with it,
/// and Alt while `flags` does, as real keys would: the events the driver sends change none of its
/// state, and the app's ⌃Tab walk lasts while the server says Control is held
/// (gpui_linux::watch_control), as a menu shows ⌥'s alternates while it says Alt is.
/// Through XTEST, the keys themselves stay down, unless `flags` holds a modifier, which the lock
/// still does till the drive moves it: the lock first, so the modifier is never up between them.
fn hold_modifiers() -> Option<()> {
    let control = (FLAGS.get() | KEY_FLAGS.get()) & u16::from(KeyButMask::CONTROL);
    if FLAGS.get() == 0 {
        xtest_modifiers(control)?;
        return lock_modifiers(0);
    }
    lock_modifiers(control | (FLAGS.get() & u16::from(KeyButMask::MOD1)))?;
    match XTEST_MODIFIERS.get() {
        0 => Some(()),
        _ => xtest_modifiers(0),
    }
}

/// Has the X server's keyboard lock `held`'s Control and Alt, and no others of them.
fn lock_modifiers(held: u16) -> Option<()> {
    use x11rb::protocol::xkb::{self, ConnectionExt as _};
    if held == LOCKED.get() {
        return Some(());
    }
    let x = server()?;
    let (connection, _) = &*x;
    // The server takes no other XKB request from a client before this one.
    connection.xkb_use_extension(1, 0).ok()?.reply().ok()?;
    let affected = xproto::ModMask::CONTROL | xproto::ModMask::M1;
    let none = xproto::ModMask::from(0u16);
    (connection.xkb_latch_lock_state(
        xkb::ID::USE_CORE_KBD.into(),
        affected,
        xproto::ModMask::from(held),
        false,
        xkb::Group::M1,
        none,
        false,
        0,
    ))
    .ok()?;
    connection.flush().ok()?;
    LOCKED.set(held);
    Some(())
}

/// Lets go of the Control and Alt the X server's keyboard still holds from an earlier launch's
/// driver (hold_modifiers), and of the keys and buttons it left down through XTEST. The server
/// keeps a lock its client leaves, and a key or button XTEST pressed, so a launch that quit or
/// crashed after a key with ⌘ or ⌃ left the next launch on the same server reading every key
/// typed as a shortcut, in its UI, which takes the server's locked modifiers into each key, and in
/// pages that get keys through XTest, as xdotool's (#2511).
fn release_held_modifiers() {
    if crate::drive_wayland::on() {
        return;
    }
    // Before the first command the driver holds nothing. With what the server holds unknown,
    // lock_modifiers tells it so, whatever it had locked.
    LOCKED.set(u16::MAX);
    lock_modifiers(0);
    let release = || -> Option<()> {
        let x = server()?;
        let (connection, root) = &*x;
        let keys = connection.query_keymap().ok()?.reply().ok()?.keys;
        let down = (0..=255u8).filter(|&code| keys[usize::from(code / 8)] & 1 << (code % 8) != 0);
        for code in down {
            xtest_event(xproto::KEY_RELEASE_EVENT, code, 0, 0)?;
        }
        let pointer = connection.query_pointer(*root).ok()?.reply().ok()?;
        let buttons = u16::from(pointer.mask);
        for button in (1..=5).filter(|&button| buttons & button_mask(button) != 0) {
            xtest_event(
                xproto::BUTTON_RELEASE_EVENT,
                button,
                pointer.root_x,
                pointer.root_y,
            )?;
        }
        Some(())
    };
    release();
}

/// Runs `key`, a key command, once the X input focus is in a browser window, looking each
/// millisecond until `until`: GLib's timeouts come before Chromium's posted tasks, so the key comes
/// with the focus, before what Chromium posts on reading it.
fn key_on_return(key: String, until: std::time::Instant, done: Box<dyn FnOnce()>) {
    let back = matches!(keyboard_window(), Some(Keyboard::Browser(..)));
    if !back && std::time::Instant::now() < until {
        after(1, Box::new(move || key_on_return(key, until, done)));
        return;
    }
    match back {
        true => {
            run(&key);
        }
        false => log!("browser: drive: the keyboard didn't come back to a browser window"),
    }
    done();
}

/// The key `code`'s events of `kinds`, with the modifiers `state`, as `key_events` does: through
/// XTEST, with the modifier keys `state` has going down before the press, unless it's a repeat, a
/// chord with Alt, or `flags` holds a modifier.
fn code_events(code: u8, state: u16, kinds: &[u8]) -> Option<()> {
    // A held key's repeat, a press of a key down already, is what XTEST can't send: the server
    // makes repeats only of a key held past its delay, which is a wait on time.
    let repeat =
        kinds == [xproto::KEY_PRESS_EVENT] && XTEST_KEYS.with_borrow(|down| down.contains(&code));
    // A chord with Alt, the Mac's ⌃ or ⌥, is slice 2's class (#2721): under Xvfb Openbox takes
    // Ctrl+Alt with an arrow for itself, so it stays sent.
    let alt = state & u16::from(KeyButMask::MOD1) != 0;
    if repeat || alt || FLAGS.get() != 0 {
        return sent_code_events(code, state, kinds);
    }
    let held = (FLAGS.get() | KEY_FLAGS.get()) & u16::from(KeyButMask::CONTROL);
    if kinds.first() == Some(&xproto::KEY_PRESS_EVENT) {
        xtest_modifiers(state & MODIFIERS | held)?;
    }
    for &kind in kinds {
        xtest_event(kind, code, 0, 0)?;
        XTEST_KEYS.with_borrow_mut(|down| {
            down.retain(|&key| key != code);
            if kind == xproto::KEY_PRESS_EVENT {
                down.push(code);
            }
        });
    }
    if kinds.last() == Some(&xproto::KEY_RELEASE_EVENT) {
        xtest_modifiers(held)?;
    }
    Some(())
}

/// Sends the key `code`'s events of `kinds`, with the modifiers `state`, to the window the X
/// server would pick.
fn sent_code_events(code: u8, state: u16, kinds: &[u8]) -> Option<()> {
    let x = server()?;
    let (connection, root) = &*x;
    // As the X server picks it: the window under the pointer when that's shown and in the focus
    // window, else the focus window. A find bar closed by a click is still the last one hovered.
    let window = match keyboard_window() {
        Some(
            Keyboard::Overlay(focus) | Keyboard::Settings(_, focus) | Keyboard::Browser(_, focus),
        ) => (HOVERED.get())
            .map(|hovered| hovered.window)
            .filter(|&hovered| viewable(connection, hovered) && within(connection, hovered, focus))
            .unwrap_or(focus),
        // A window of Chromium's own, such as the Task Manager, else none of the app's has it.
        None => chromium_focus().or_else(|| driven().map(|browser| browser.window))?,
    };
    for &kind in kinds {
        let event = KeyPressEvent {
            response_type: kind,
            detail: code,
            sequence: 0,
            time: x11rb::CURRENT_TIME,
            root: *root,
            event: window,
            child: x11rb::NONE,
            root_x: 0,
            root_y: 0,
            event_x: 0,
            event_y: 0,
            state: KeyButMask::from(state),
            same_screen: true,
        };
        // A release goes to every client listening for one in the window, as the X server sends
        // it, and so to the app's watch on a held Escape (held_escape.rs) too.
        let mask = match kind {
            xproto::KEY_RELEASE_EVENT => EventMask::KEY_RELEASE,
            _ => EventMask::NO_EVENT,
        };
        connection.send_event(false, window, mask, event).ok()?;
    }
    connection.flush().ok()
}

/// Two numbers from `words`.
pub(super) fn point<'a>(words: &mut impl Iterator<Item = &'a str>) -> Option<(f64, f64)> {
    Some((words.next()?.parse().ok()?, words.next()?.parse().ok()?))
}

/// The browser window `window` is, as order and front name it: "window <n>", numbered as the
/// window command numbers them.
fn name_of(browser: &Browser) -> String {
    let index = MADE.with_borrow(|made| made.iter().position(|&(id, _)| id == browser.id));
    index.map_or_else(
        || "none".to_string(),
        |index| format!("window {}", index + 1),
    )
}

/// Logs the windows on screen over the driven browser window and overlapping it, front first, down
/// to it, as the X server stacks them and Windows' driver lists them: a browser window by what
/// order calls it, another of the app's as "ilo" and its title, and another app's as "other" and
/// its title.
fn stack() -> Option<()> {
    let driven = driven()?;
    let x = server()?;
    let (connection, root) = &*x;
    let tops = connection.query_tree(*root).ok()?.reply().ok()?.children;
    let bounds = |window| {
        let geometry = connection.get_geometry(window).ok()?.reply().ok()?;
        let [x, y] = [geometry.x, geometry.y].map(i32::from);
        Some([
            x,
            y,
            x + i32::from(geometry.width),
            y + i32::from(geometry.height),
        ])
    };
    let own = tops
        .iter()
        .copied()
        .find(|&top| within(connection, driven.window, top))?;
    let over = bounds(own)?;
    let browsers: Vec<Browser> = (0..MADE.with_borrow(Vec::len) + WINDOWS.with_borrow(Vec::len))
        .filter_map(browser)
        .collect();
    let mut stack = Vec::new();
    for &top in tops.iter().rev() {
        if top == own {
            stack.push("driven".to_string());
            break;
        }
        let Some(at) = bounds(top) else { continue };
        if !viewable(connection, top)
            || at[0] >= over[2]
            || over[0] >= at[2]
            || at[1] >= over[3]
            || over[1] >= at[3]
        {
            continue;
        }
        if let Some(browser) = browsers.iter().find(|b| within(connection, b.window, top)) {
            stack.push(name_of(browser));
            continue;
        }
        // The window manager's frame holds the client, which has the title and process.
        let mut clients = vec![top];
        clients.extend(connection.query_tree(top).ok()?.reply().ok()?.children);
        let property = |window, name: &str| {
            let reply = connection
                .get_property(false, window, atom(name)?, xproto::AtomEnum::ANY, 0, 256)
                .ok()?
                .reply()
                .ok()?;
            (!reply.value.is_empty()).then_some(reply)
        };
        let title = clients.iter().find_map(|&client| {
            let reply = property(client, "_NET_WM_NAME").or_else(|| property(client, "WM_NAME"))?;
            Some(String::from_utf8_lossy(&reply.value).into_owned())
        });
        let pid = clients
            .iter()
            .find_map(|&client| property(client, "_NET_WM_PID")?.value32()?.next());
        let whose = if pid == Some(std::process::id()) {
            "ilo"
        } else {
            "other"
        };
        stack.push(format!("{whose} {}", title.unwrap_or_default()));
    }
    log!("browser: stack {}", stack.join(", "));
    Some(())
}

/// The browser windows that are open, front to back, as the X server stacks them.
fn stacked() -> Option<Vec<Browser>> {
    let x = server()?;
    let (connection, root) = &*x;
    let browsers: Vec<Browser> = (0..MADE.with_borrow(Vec::len) + WINDOWS.with_borrow(Vec::len))
        .filter_map(browser)
        .collect();
    let tops = connection.query_tree(*root).ok()?.reply().ok()?.children;
    Some(
        (tops.iter().rev())
            .filter_map(|&top| {
                browsers
                    .iter()
                    .find(|b| within(connection, b.window, top))
                    .copied()
            })
            .collect(),
    )
}

/// Whether (`x`, `y`) in browser window `id`'s content is on one of its pages.
/// shortcut: where the shell lays them out, as `pages` has them.
pub(super) fn over_page(id: bc::bc_id, x: f64, y: f64) -> bool {
    let shell =
        WINDOWS.with_borrow(|windows| windows.iter().find(|w| w.id == id).map(|w| w.shell.clone()));
    let (Some(shell), Some(cx)) = (shell, CX.with_borrow(Clone::clone)) else {
        return false;
    };
    let at = gpui::point(gpui::px(x as f32), gpui::px(y as f32));
    cx.update(|cx| {
        let Some(shell) = shell.upgrade() else {
            return false;
        };
        shell
            .read(cx)
            .shown_pages()
            .iter()
            .any(|(_, page)| page.contains(&at))
    })
}

/// The pages of browser window `id`, as drive.mm lists their views: from the bottom-left corner of
/// content `width` by `height`, with the radius of their corners if they're round, and a pane's
/// docked DevTools before its page, as the core reports them.
/// shortcut: where the shell lays them out and the radius it gives their corners, which the core
/// follows. They're Aura's windows in Chromium's one X window, and no core call says where they
/// are or how round. At the content's size now, which a resize lays the pages out at before the
/// shell draws there: at the size it last drew, a page full screen in a window that had just
/// grown read as the window before (#1903).
pub(super) fn pages(id: bc::bc_id, width: f64, height: f64) -> Option<String> {
    let shell = WINDOWS
        .with_borrow(|windows| windows.iter().find(|w| w.id == id).map(|w| w.shell.clone()))?;
    let cx = CX.with_borrow(Clone::clone)?;
    let viewport = gpui::size(gpui::px(width as f32), gpui::px(height as f32));
    let (pages, (_, radius)) = cx.update(|cx| {
        let shell = shell.upgrade()?;
        let shell = shell.read(cx);
        Some((shell.shown_pages_at(viewport), shell.page_frame(viewport)))
    })?;
    let docked = devtools(id);
    let frames = pages.into_iter().flat_map(|(_, page)| {
        let page = [page.left(), page.top(), page.size.width, page.size.height].map(f64::from);
        // DevTools fill the pane they're docked in.
        match docked {
            Some((devtools, beside)) if (0..4).all(|i| (devtools[i] - page[i]).abs() <= 1.) => {
                vec![devtools, beside]
            }
            _ => vec![page],
        }
    });
    let views = frames.map(|[x, top, width, high]| {
        let y = height - (top + high);
        let round = match radius > 0. {
            true => format!(" radius {radius:.0}"),
            false => String::new(),
        };
        format!(", WebContentsViewCocoa {x:.0},{y:.0} {width:.0}x{high:.0}{round}")
    });
    Some(views.collect())
}

/// Where DevTools docked in a page of `id` show, and the page beside them, from the content's
/// top-left corner, which the core reports, as the X server can't see them.
fn devtools(id: bc::bc_id) -> Option<([f64; 4], [f64; 4])> {
    let mut devtools = bc::bc_rect {
        x: 0.,
        y: 0.,
        width: 0.,
        height: 0.,
    };
    let mut page = devtools;
    unsafe { bc::bc_window_devtools(id, &mut devtools, &mut page) }.then_some(())?;
    let frame = |r: bc::bc_rect| [r.x, r.y, r.width, r.height];
    Some((frame(devtools), frame(page)))
}

/// The ring the core draws inside a split view's focused page, over the pages, as the Mac's driver
/// lists its view: from the content's bottom-left corner, as the pages are.
pub(super) fn ring(id: bc::bc_id, height: f64) -> Option<String> {
    let mut frame = bc::bc_rect {
        x: 0.,
        y: 0.,
        width: 0.,
        height: 0.,
    };
    unsafe { bc::bc_window_focus_ring(id, &mut frame) }.then_some(())?;
    let bc::bc_rect {
        x,
        y,
        width,
        height: high,
    } = frame;
    let y = height - (y + high);
    Some(format!(
        ", BrowserCoreFocusRing {x:.0},{y:.0} {width:.0}x{high:.0}"
    ))
}

/// The page of `id` with the keyboard, by its pane's frame from the bottom-left corner of content
/// `height` high, as the pages are, which tells a split view's pages apart, or "none".
pub(super) fn keyboard_page(id: bc::bc_id, height: f64) -> String {
    let mut page = bc::bc_rect {
        x: 0.,
        y: 0.,
        width: 0.,
        height: 0.,
    };
    if !unsafe { bc::bc_window_keyboard_page(id, &mut page) } {
        return "none".into();
    }
    let bc::bc_rect {
        x,
        y,
        width,
        height: high,
    } = page;
    format!("{x:.0},{:.0} {width:.0}x{high:.0}", height - (y + high))
}

/// A tab's dialog over `id`, such as print preview, from the content's top-left corner, and its
/// size, as the Mac's driver lists a child window.
pub(super) fn dialog(id: bc::bc_id) -> Option<String> {
    let mut frame = bc::bc_rect {
        x: 0.,
        y: 0.,
        width: 0.,
        height: 0.,
    };
    unsafe { bc::bc_window_dialog(id, &mut frame) }.then_some(())?;
    let bc::bc_rect {
        x,
        y,
        width,
        height,
    } = frame;
    Some(format!("{x:.0},{y:.0} {width:.0}x{height:.0}"))
}

/// The centres of the window buttons the UI draws for `id`, from the window's top-left corner, as
/// the Mac's driver gives AppKit's, or "hidden" for each.
pub(super) fn buttons(id: bc::bc_id) -> Option<String> {
    let shell = WINDOWS
        .with_borrow(|windows| windows.iter().find(|w| w.id == id).map(|w| w.shell.clone()))?;
    let cx = CX.with_borrow(Clone::clone)?;
    let drawn = cx.update(|cx| Some(shell.upgrade()?.read(cx).draws_own_window_buttons()))?;
    let each = browser::shell::WINDOW_BUTTONS.map(|(x, _)| match drawn {
        true => format!("{x:.1},{:.1}", browser::tokens::shell::WINDOW_BUTTON_Y),
        false => "hidden".to_string(),
    });
    Some(each.join(" "))
}

/// `window`'s frame in points, from the root's top-left corner: x, y, width and height.
fn frame(window: xproto::Window) -> Option<[f64; 4]> {
    let x = server()?;
    let (connection, root) = &*x;
    let geometry = connection.get_geometry(window).ok()?.reply().ok()?;
    let at = connection
        .translate_coordinates(window, *root, 0, 0)
        .ok()?
        .reply()
        .ok()?;
    let scale = scale();
    Some(
        [
            at.dst_x.into(),
            at.dst_y.into(),
            geometry.width.into(),
            geometry.height.into(),
        ]
        .map(|n: f64| n / scale),
    )
}

/// The text property `name` of `window`, as UTF-8.
fn text_property(window: xproto::Window, name: &str) -> Option<String> {
    let x = server()?;
    let (connection, _) = &*x;
    let atom = connection
        .intern_atom(false, name.as_bytes())
        .ok()?
        .reply()
        .ok()?
        .atom;
    let property = (connection.get_property(false, window, atom, xproto::AtomEnum::ANY, 0, 1024))
        .ok()?
        .reply()
        .ok()?;
    String::from_utf8(property.value).ok()
}

/// The window with the input focus while it's in one of this process's that isn't the app's own,
/// such as Chromium's Task Manager, by the process id its window manager hint names.
fn chromium_focus() -> Option<xproto::Window> {
    let x = server()?;
    let (connection, _) = &*x;
    let pid = connection
        .intern_atom(false, b"_NET_WM_PID")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let focus = input_focus()?;
    let mut window = focus;
    loop {
        let property = connection
            .get_property(false, window, pid, xproto::AtomEnum::CARDINAL, 0, 1)
            .ok()?
            .reply()
            .ok()?;
        if let Some(owner) = property.value32().and_then(|mut values| values.next()) {
            return (owner == std::process::id()).then_some(focus);
        }
        let tree = connection.query_tree(window).ok()?.reply().ok()?;
        if window == tree.root || tree.parent == x11rb::NONE {
            return None;
        }
        window = tree.parent;
    }
}

/// The title of the window with the keyboard.
fn key_title() -> Option<String> {
    if crate::drive_wayland::on() {
        return crate::drive_wayland::key_title();
    }
    Some(match keyboard_window() {
        Some(Keyboard::Overlay(window) | Keyboard::Settings(window, _)) => {
            text_property(window, "_NET_WM_NAME").unwrap_or_default()
        }
        // A dialog of Chromium's in it is an aura window, which the X server has no title for, so
        // the core gives it.
        Some(Keyboard::Browser(browser, _))
            if unsafe { bc::bc_window_keyboard_in_child(browser.id) } =>
        {
            child_title(browser.id)
        }
        Some(Keyboard::Browser(browser, _)) => {
            text_property(browser.window, "_NET_WM_NAME").unwrap_or_default()
        }
        // A window of Chromium's own, such as the Task Manager.
        None => focus_title()?,
    })
}

/// The title of the dialog of Chromium's with the keyboard in browser window `id`.
pub(super) fn child_title(id: bc::bc_id) -> String {
    let length = unsafe { bc::bc_window_child_title(id, std::ptr::null_mut(), 0) };
    let mut title = vec![0u8; length + 1];
    let (to, capacity) = (title.as_mut_ptr().cast(), title.len());
    unsafe { bc::bc_window_child_title(id, to, capacity) };
    title.truncate(length);
    String::from_utf8_lossy(&title).into_owned()
}

/// Logs the title of the window with the keyboard once it has `text` in it, as a window that takes
/// the keyboard a moment after a command does, for up to 10 s as drive.mm's AwaitKeyTitle, then
/// reports command `n` done.
fn await_key_title(n: u64, text: String, tries: u32) {
    let title = key_title().unwrap_or_default();
    if title.contains(&text) {
        log!("browser: key window titled {title}");
    } else if tries > 0 {
        after(100, Box::new(move || await_key_title(n, text, tries - 1)));
        return;
    } else {
        log!("browser: no key window titled {text}, the key window's is {title}");
    }
    settle(n);
}

/// Reports command `n` done once `window` has the keyboard, which the window manager gives it a
/// moment after it's asked, or at `until`. A deadline, as a busy main loop runs the timer late.
fn await_focus(n: u64, window: xproto::Window, until: std::time::Instant) {
    let has = server().is_some_and(|x| input_focus().is_some_and(|f| within(&x.0, f, window)));
    if !has && std::time::Instant::now() < until {
        after(100, Box::new(move || await_focus(n, window, until)));
        return;
    }
    if !has {
        log!("browser: drive {n}: the window manager didn't give the window the keyboard");
    }
    settle(n);
}

/// Reports command `n` done once the UI's window `gpui` is `size` in points, as the UI has then
/// laid out at that size and placed what floats over it, or at `until`.
/// Has the command wait, before it's done, for the UI's view `view` to be `width` by `height`.
pub(super) fn resizing(view: usize, width: f64, height: f64) {
    RESIZING.set(Some((view, width, height)));
}

fn await_size(n: u64, gpui: usize, size: (f64, f64), until: std::time::Instant) {
    let has = ui_size(gpui).is_some_and(|(width, height)| {
        (f64::from(width) - size.0).abs() < 1. && (f64::from(height) - size.1).abs() < 1.
    });
    if !has && std::time::Instant::now() < until {
        after(10, Box::new(move || await_size(n, gpui, size, until)));
        return;
    }
    if !has {
        let has = ui_size(gpui).map_or("none".into(), |(w, h)| format!("{w}x{h}"));
        log!(
            "browser: drive {n}: the UI isn't {}x{} yet, it's {has}",
            size.0,
            size.1
        );
    }
    settle(n);
}

/// The size GPUI has for its window whose view is `gpui`, in points.
fn ui_size(gpui: usize) -> Option<(f32, f32)> {
    let cx = CX.with_borrow(Clone::clone)?;
    cx.update(|cx| {
        cx.windows().into_iter().find_map(|handle| {
            let size = handle.update(cx, |_, window, _| {
                let ours = crate::ns_view(window) as usize == gpui;
                ours.then(|| window.viewport_size())
            });
            size.ok().flatten()
        })
    })
    .map(|size| (f32::from(size.width), f32::from(size.height)))
}

/// The title of the window with the input focus, or of the first window it's in that has one.
fn focus_title() -> Option<String> {
    let x = server()?;
    let (connection, _) = &*x;
    let mut window = input_focus()?;
    loop {
        if let Some(title) = text_property(window, "_NET_WM_NAME").filter(|t| !t.is_empty()) {
            return Some(title);
        }
        let tree = connection.query_tree(window).ok()?.reply().ok()?;
        if window == tree.root || tree.parent == x11rb::NONE {
            return None;
        }
        window = tree.parent;
    }
}

/// Whether the window manager has `window` full screen.
fn full_screen(window: xproto::Window) -> Option<bool> {
    let x = server()?;
    let (connection, _) = &*x;
    let [state, full] = ["_NET_WM_STATE", "_NET_WM_STATE_FULLSCREEN"].map(|name| {
        connection
            .intern_atom(false, name.as_bytes())
            .ok()?
            .reply()
            .ok()
            .map(|r| r.atom)
    });
    let property = (connection.get_property(false, window, state?, xproto::AtomEnum::ATOM, 0, 64))
        .ok()?
        .reply()
        .ok()?;
    let full = full?;
    Some(property.value32()?.any(|atom| atom == full))
}

/// Whether the window manager has `window` maximized, both ways.
fn maximized(window: xproto::Window) -> Option<bool> {
    let x = server()?;
    let (connection, _) = &*x;
    let [state, vertical, horizontal] = [
        "_NET_WM_STATE",
        "_NET_WM_STATE_MAXIMIZED_VERT",
        "_NET_WM_STATE_MAXIMIZED_HORZ",
    ]
    .map(atom);
    let property = (connection.get_property(false, window, state?, xproto::AtomEnum::ATOM, 0, 64))
        .ok()?
        .reply()
        .ok()?;
    let atoms: Vec<_> = property
        .value32()
        .map(Iterator::collect)
        .unwrap_or_default();
    Some(atoms.contains(&vertical?) && atoms.contains(&horizontal?))
}

/// Logs "browser: window zoomed <0|1>" as the window manager maximizes `window` or restores it, as
/// drive.mm logs AppKit's zoom. On a connection of the driver's own, whose thread waits on the X
/// server's property changes, so it wakes only for one.
fn watch_zoom(window: xproto::Window) {
    static WATCHER: OnceLock<Option<Arc<RustConnection>>> = OnceLock::new();
    let watcher = WATCHER.get_or_init(|| {
        let (connection, _) = x11rb::connect(None).ok()?;
        let connection = Arc::new(connection);
        let reader = connection.clone();
        std::thread::spawn(move || zoom_changes(&reader));
        Some(connection)
    });
    let Some(connection) = watcher else { return };
    let aux = ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE);
    // A window that has closed since answers with an error, which is dropped.
    if let Ok(cookie) = connection.change_window_attributes(window, &aux) {
        cookie.ignore_error();
    }
    connection.flush().ok();
}

fn zoom_changes(connection: &RustConnection) {
    let atom = |name: &str| {
        let cookie = connection.intern_atom(false, name.as_bytes()).ok()?;
        cookie.reply().ok().map(|reply| reply.atom)
    };
    let names = [
        "_NET_WM_STATE",
        "_NET_WM_STATE_MAXIMIZED_VERT",
        "_NET_WM_STATE_MAXIMIZED_HORZ",
    ];
    let [Some(state), Some(vertical), Some(horizontal)] = names.map(atom) else {
        return;
    };
    let mut zoomed = HashMap::new();
    while let Ok(event) = connection.wait_for_event() {
        let Event::PropertyNotify(notify) = event else {
            continue;
        };
        if notify.atom != state {
            continue;
        }
        let atom_type = xproto::AtomEnum::ATOM;
        let property = connection.get_property(false, notify.window, state, atom_type, 0, 64);
        let Some(property) = property.ok().and_then(|cookie| cookie.reply().ok()) else {
            continue;
        };
        let atoms: Vec<_> = property
            .value32()
            .map(Iterator::collect)
            .unwrap_or_default();
        let now = atoms.contains(&vertical) && atoms.contains(&horizontal);
        if zoomed.insert(notify.window, now).unwrap_or(false) != now {
            log!("browser: window zoomed {}", now as u8);
        }
    }
}

/// Chromium's own windows over browser window `browser`, such as autofill's dropdown, which
/// drive.mm lists among a window's child windows: those that show and that no window manager takes,
/// from the bottom up. Chromium's are the X client's that made `browser`, as a window's id says,
/// where GPUI's, such as the overlays' shadows, are another client's.
fn popups(browser: xproto::Window) -> Option<Vec<xproto::Window>> {
    let x = server()?;
    let (connection, root) = &*x;
    let client = !connection.setup().resource_id_mask;
    let tops = connection.query_tree(*root).ok()?.reply().ok()?.children;
    Some(
        tops.into_iter()
            .filter(|&top| top != browser && top & client == browser & client)
            .filter(|&top| {
                connection
                    .get_window_attributes(top)
                    .ok()
                    .and_then(|cookie| cookie.reply().ok())
                    .is_some_and(|a| {
                        a.override_redirect && a.map_state == xproto::MapState::VIEWABLE
                    })
            })
            .collect(),
    )
}

/// The item of `menus` titled `title`, in a submenu too, or with "Submenu › Item", the item in the
/// submenu so titled.
fn menu_item<'a>(menus: &'a [Menu], title: &str) -> Option<&'a MenuItem> {
    let (parent, item_title) = title
        .split_once(" › ")
        .map_or((None, title), |(p, i)| (Some(p), i));
    menus
        .iter()
        .find_map(|menu| find_item(&menu.items, parent, item_title, parent.is_none()))
}

fn find_item<'a>(
    items: &'a [MenuItem],
    parent: Option<&str>,
    title: &str,
    inside: bool,
) -> Option<&'a MenuItem> {
    items.iter().find_map(|item| match item {
        MenuItem::Action { name, .. } if inside && name.as_ref() == title => Some(item),
        MenuItem::Submenu(menu) => {
            let inside = inside || parent == Some(menu.name.as_ref());
            find_item(&menu.items, parent, title, inside)
        }
        _ => None,
    })
}

/// A drag from another app, as dragover, dragdrop and dragleave play it: XDND's source
/// (freedesktop.org's, version 5), with a window of the driver's own that owns the drag's
/// selection, handing the drag to the browser window's top-level window, or the proxy that names,
/// as another app's would.
struct Source {
    window: xproto::Window,
    /// What it carries, in each type it offers.
    data: Vec<(xproto::Atom, Vec<u8>)>,
    /// The window its messages name, and the one they go to, its XdndProxy if it has one.
    target: Option<(xproto::Window, xproto::Window)>,
    /// The view under it, as the Mac's driver names AppKit's.
    view: &'static str,
    waiting: Option<Waiting>,
}

/// What the drag waits for its target to say.
#[derive(Clone, Copy)]
enum Waiting {
    /// Whether it takes the drag there, and whether to let go once it says.
    Status(bool, std::time::Instant),
    /// Whether it took the drop.
    Finished(std::time::Instant),
}

thread_local! {
    static SOURCE: RefCell<Option<Source>> = const { RefCell::new(None) };
    /// The driver's window, which owns a drag's selection and is handed the clipboard's text,
    /// made once.
    static DRIVER_WINDOW: Cell<Option<xproto::Window>> = const { Cell::new(None) };
    /// Since when `pasteboard` waits for the clipboard's owner to hand its text to the driver.
    static PASTE_ASKED: Cell<Option<std::time::Instant>> = const { Cell::new(None) };
}

fn atom(name: &str) -> Option<xproto::Atom> {
    let x = server()?;
    Some(
        x.0.intern_atom(false, name.as_bytes())
            .ok()?
            .reply()
            .ok()?
            .atom,
    )
}

/// Maps a desktop window over the whole screen, as a desktop shell (Plasma's, xfdesktop) does, or
/// takes it away: KWin shows the desktop by raising that window over the others and giving it the
/// keyboard, and without one it shows nothing.
fn desktop_shell(on: bool) -> Option<()> {
    thread_local! {
        static SHELL: Cell<Option<xproto::Window>> = const { Cell::new(None) };
    }
    let x = server()?;
    let (connection, root) = &*x;
    if let Some(window) = SHELL.take() {
        connection.destroy_window(window).ok()?;
    }
    if on {
        let screen = (connection.setup().roots.iter()).find(|screen| screen.root == *root)?;
        let window = connection.generate_id().ok()?;
        (connection.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            *root,
            0,
            0,
            screen.width_in_pixels,
            screen.height_in_pixels,
            0,
            xproto::WindowClass::INPUT_OUTPUT,
            x11rb::COPY_FROM_PARENT,
            &xproto::CreateWindowAux::new().background_pixel(screen.black_pixel),
        ))
        .ok()?;
        let kind = atom("_NET_WM_WINDOW_TYPE")?;
        let desktop = atom("_NET_WM_WINDOW_TYPE_DESKTOP")?;
        (connection.change_property32(
            xproto::PropMode::REPLACE,
            window,
            kind,
            xproto::AtomEnum::ATOM,
            &[desktop],
        ))
        .ok()?;
        connection.map_window(window).ok()?;
        SHELL.set(Some(window));
    }
    connection.flush().ok()
}

/// The driver's own window, on its connection, which nobody sees.
fn driver_window() -> Option<xproto::Window> {
    if let Some(window) = DRIVER_WINDOW.get() {
        return Some(window);
    }
    let x = server()?;
    let (connection, root) = &*x;
    let window = connection.generate_id().ok()?;
    (connection.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        window,
        *root,
        -1,
        -1,
        1,
        1,
        0,
        xproto::WindowClass::INPUT_ONLY,
        x11rb::COPY_FROM_PARENT,
        &xproto::CreateWindowAux::new(),
    ))
    .ok()?;
    DRIVER_WINDOW.set(Some(window));
    Some(window)
}

/// Asks the clipboard's owner for its text, on the driver's connection, which `sync` reads the
/// answer from: the owner may be Chromium, such as DevTools after ⌘C, which answers on the main
/// thread that GPUI's own read would block.
fn ask_clipboard() -> Option<()> {
    let x = server()?;
    let (connection, _) = &*x;
    let window = driver_window()?;
    let names = ["CLIPBOARD", "UTF8_STRING", "BROWSER_DRIVE_PASTEBOARD"];
    let [clipboard, utf8, into] = names.map(atom);
    (connection.convert_selection(window, clipboard?, utf8?, into?, x11rb::CURRENT_TIME)).ok()?;
    connection.flush().ok()?;
    PASTE_ASKED.set(Some(std::time::Instant::now()));
    Some(())
}

/// Logs the clipboard's text the owner handed over, or none if it had none.
fn pasted(notify: &xproto::SelectionNotifyEvent) {
    if PASTE_ASKED.get().is_none() || Some(notify.requestor) != DRIVER_WINDOW.get() {
        return;
    }
    PASTE_ASKED.set(None);
    let read = || {
        let x = server()?;
        let (connection, _) = &*x;
        let any = xproto::AtomEnum::ANY;
        let (window, property) = (notify.requestor, notify.property);
        let reply = connection.get_property(true, window, property, any, 0, 1 << 20);
        Some(String::from_utf8_lossy(&reply.ok()?.reply().ok()?.value).into_owned())
    };
    let text = (notify.property != x11rb::NONE).then(read).flatten();
    log!("browser: pasteboard text {}", text.unwrap_or_default());
}

/// Whether `pasteboard` still waits for the clipboard's owner, which gets 4 s, as GPUI's own read
/// gives it, before the driver logs no text.
fn paste_waits() -> bool {
    let Some(since) = PASTE_ASKED.get() else {
        return false;
    };
    if since.elapsed() < std::time::Duration::from_secs(4) {
        return true;
    }
    PASTE_ASKED.set(None);
    log!("browser: pasteboard text ");
    false
}

/// `path` as a file: URL's path, its bytes outside the unreserved ones escaped.
pub(super) fn file_url(path: &str) -> String {
    let mut url = String::from("file://");
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                url.push(byte as char)
            }
            _ => url.push_str(&format!("%{byte:02X}")),
        }
    }
    url
}

/// Starts a drag of `kind` carrying `value`, as drive.mm's pasteboard holds it: a URL or a file's
/// as a URI list, a URL as text too, and text as text.
fn start_drag(kind: &str, value: &str) -> Option<Source> {
    let x = server()?;
    let (connection, _) = &*x;
    let [uris, utf8, plain] =
        ["text/uri-list", "UTF8_STRING", "text/plain;charset=utf-8"].map(atom);
    let data = match kind {
        "url" => vec![
            (uris?, format!("{value}\r\n").into_bytes()),
            (utf8?, value.into()),
        ],
        "file" => vec![(uris?, format!("{}\r\n", file_url(value)).into_bytes())],
        "text" => vec![(utf8?, value.into()), (plain?, value.into())],
        // As host_drag_tab carries one from another of the app's windows.
        "tab" => vec![(
            atom("application/x-ilo-tab")?,
            format!("{} {value}", std::process::id()).into_bytes(),
        )],
        _ => return None,
    };
    let window = driver_window()?;
    let types: Vec<u32> = data.iter().map(|(kind, _)| *kind).collect();
    (connection.change_property(
        xproto::PropMode::REPLACE,
        window,
        atom("XdndTypeList")?,
        xproto::AtomEnum::ATOM,
        32,
        types.len() as u32,
        &types
            .iter()
            .flat_map(|atom| atom.to_ne_bytes())
            .collect::<Vec<u8>>(),
    ))
    .ok()?;
    (connection.set_selection_owner(window, atom("XdndSelection")?, x11rb::CURRENT_TIME)).ok()?;
    connection.flush().ok()?;
    Some(Source {
        window,
        data,
        target: None,
        view: "nothing",
        waiting: None,
    })
}

fn send_xdnd(to: xproto::Window, named: xproto::Window, kind: &str, data: [u32; 5]) -> Option<()> {
    let x = server()?;
    let message = xproto::ClientMessageEvent::new(32, named, atom(kind)?, data);
    x.0.send_event(false, to, EventMask::NO_EVENT, message)
        .ok()?;
    x.0.flush().ok()
}

/// Moves the drag to (`x`, `y`) in the driven browser window, starting it with what `carried` says
/// if there's none, and has the target say whether it takes it there, then with `drop` let go.
fn drag_over(x: f64, y: f64, carried: Option<(&str, &str)>, drop: bool) -> Option<()> {
    let browser = driven()?;
    let at = target_in(browser.window, x, y)?;
    let mut source = match SOURCE.take() {
        Some(source) => source,
        None => {
            let (kind, value) = carried?;
            start_drag(kind, value)?
        }
    };
    let x11 = server()?;
    let (connection, _) = &*x11;
    // AppKit's names for the views a drag there goes to: the UI's, and the page's.
    let child = (connection.translate_coordinates(browser.window, browser.window, at.x, at.y))
        .ok()?
        .reply()
        .ok()?
        .child;
    source.view = match child == browser.ui {
        true => "HostUIView",
        false => "WebContentsViewCocoa",
    };
    let property = |window, name: &str, kind: xproto::AtomEnum| {
        let reply = (connection.get_property(false, window, atom(name)?, kind, 0, 1))
            .ok()?
            .reply()
            .ok()?;
        reply.value32()?.next()
    };
    let named = browser.window;
    let to = property(named, "XdndProxy", xproto::AtomEnum::WINDOW).unwrap_or(named);
    if property(to, "XdndAware", xproto::AtomEnum::ATOM).is_none() {
        log!("browser: drag over {} refused", source.view);
        SOURCE.set(Some(source));
        return Some(());
    }
    if source.target != Some((named, to)) {
        if let Some((named, to)) = source.target {
            send_xdnd(to, named, "XdndLeave", [source.window, 0, 0, 0, 0])?;
        }
        let types = source.data.iter().map(|(kind, _)| *kind);
        let mut offered = [0; 3];
        offered
            .iter_mut()
            .zip(types)
            .for_each(|(each, kind)| *each = kind);
        let [a, b, c] = offered;
        send_xdnd(to, named, "XdndEnter", [source.window, 5 << 24, a, b, c])?;
        source.target = Some((named, to));
    }
    let root = (u32::from(at.root_x as u16) << 16) | u32::from(at.root_y as u16);
    let copy = atom("XdndActionCopy")?;
    send_xdnd(to, named, "XdndPosition", [source.window, 0, root, 0, copy])?;
    source.waiting = Some(Waiting::Status(drop, std::time::Instant::now()));
    SOURCE.set(Some(source));
    Some(())
}

/// Takes the drag away, as cancelled.
fn drag_leave() {
    if let Some(source) = SOURCE.take()
        && let Some((named, to)) = source.target
    {
        send_xdnd(to, named, "XdndLeave", [source.window, 0, 0, 0, 0]);
    }
    end_drag();
}

fn end_drag() {
    SOURCE.set(None);
    if let Some(x) = server()
        && let Some(selection) = atom("XdndSelection")
    {
        _ =
            x.0.set_selection_owner(x11rb::NONE, selection, x11rb::CURRENT_TIME);
        _ = x.0.flush();
    }
}

/// Hands the drag's target what it carries, as `request` asks for it.
fn serve_drag(request: &xproto::SelectionRequestEvent) {
    let Some(x) = server() else { return };
    let (connection, _) = &*x;
    let data = SOURCE.with_borrow(|source| {
        (source.as_ref())
            .filter(|source| source.window == request.owner)
            .map(|source| source.data.clone())
    });
    // An old client's request names no property.
    let property = match request.property {
        x11rb::NONE => request.target,
        property => property,
    };
    let served = data.and_then(|data| {
        if Some(request.target) == atom("TARGETS") {
            let types: Vec<u8> = data
                .iter()
                .flat_map(|(kind, _)| kind.to_ne_bytes())
                .collect();
            let count = data.len() as u32;
            let atoms = xproto::AtomEnum::ATOM;
            (connection.change_property(
                xproto::PropMode::REPLACE,
                request.requestor,
                property,
                atoms,
                32,
                count,
                &types,
            ))
            .ok()
        } else {
            let (kind, bytes) = data.iter().find(|(kind, _)| *kind == request.target)?;
            let count = bytes.len() as u32;
            (connection.change_property(
                xproto::PropMode::REPLACE,
                request.requestor,
                property,
                *kind,
                8,
                count,
                bytes,
            ))
            .ok()
        }
    });
    let notify = xproto::SelectionNotifyEvent {
        response_type: xproto::SELECTION_NOTIFY_EVENT,
        sequence: 0,
        time: request.time,
        requestor: request.requestor,
        selection: request.selection,
        target: request.target,
        property: if served.is_some() {
            property
        } else {
            x11rb::NONE
        },
    };
    _ = connection.send_event(false, request.requestor, EventMask::NO_EVENT, notify);
    _ = connection.flush();
}

/// Hears the drag's target say whether it takes the drag, and whether it took the drop, and logs
/// it as drive.mm does.
fn drag_answered(message: &xproto::ClientMessageEvent) {
    let Some(mut source) = SOURCE.take() else {
        return;
    };
    let data = message.data.as_data32();
    let kind = Some(message.type_);
    let taken = data[1] & 1 == 1;
    let from_target = source.target.is_some_and(|(named, _)| named == data[0]);
    match source.waiting {
        Some(Waiting::Status(drop, _)) if from_target && kind == atom("XdndStatus") => {
            log!(
                "browser: drag over {} {}",
                source.view,
                if taken { "taken" } else { "refused" }
            );
            source.waiting = None;
            let (named, to) = source.target.unwrap_or_default();
            if drop && taken {
                send_xdnd(to, named, "XdndDrop", [source.window, 0, 0, 0, 0]);
                source.waiting = Some(Waiting::Finished(std::time::Instant::now()));
            } else if drop {
                send_xdnd(to, named, "XdndLeave", [source.window, 0, 0, 0, 0]);
                log!("browser: drag refused");
                return end_drag();
            }
        }
        Some(Waiting::Finished(_)) if from_target && kind == atom("XdndFinished") => {
            log!(
                "browser: drag {}",
                if taken { "dropped" } else { "refused" }
            );
            return end_drag();
        }
        _ => {}
    }
    SOURCE.set(Some(source));
}

/// Whether the drag waits for its target, which gets 5 s before the driver takes its silence for
/// a no, as AppKit's answer comes at once.
fn drag_waits() -> bool {
    let waiting =
        SOURCE.with_borrow(|source| source.as_ref().and_then(|s| s.waiting.map(|w| (w, s.view))));
    let Some((waiting, view)) = waiting else {
        return false;
    };
    let since = match waiting {
        Waiting::Status(_, since) | Waiting::Finished(since) => since,
    };
    if since.elapsed() < std::time::Duration::from_secs(5) {
        return true;
    }
    match waiting {
        Waiting::Status(..) => log!("browser: drag over {view} refused, unanswered"),
        Waiting::Finished(_) => log!("browser: drag refused, unanswered"),
    }
    drag_leave();
    false
}

/// Runs a command. Returns false for one it can't do.
fn run(line: &str) -> bool {
    run_command(line).is_some()
}

fn run_command(line: &str) -> Option<()> {
    let mut words = line.split_whitespace();
    let action = words.next()?;
    // What follows the command's name, for those that take text.
    let rest = line.get(action.len() + 1..).unwrap_or("");
    let none = 0;
    if crate::drive_wayland::on()
        && let Some(done) = crate::drive_wayland::run(action, line)
    {
        return done;
    }
    match action {
        "window" => {
            let number: usize = words.next()?.parse().ok()?;
            let browser = browser(number.checked_sub(1)?)?;
            let behind = match words.next() {
                None => false,
                Some("behind") => true,
                Some(_) => return None,
            };
            DRIVEN.set(number - 1);
            log!(
                "browser: drive window {number}{}",
                if behind { " behind" } else { "" }
            );
            // With behind, it stays where it is, as drive.mm's does, for another app's window over it.
            if behind {
                return Some(());
            }
            // To the front with the keyboard, as a click on it does.
            let x = server()?;
            let (connection, root) = &*x;
            let above = ConfigureWindowAux::new().stack_mode(StackMode::ABOVE);
            connection.configure_window(browser.window, &above).ok()?;
            // A window that has it keeps it where it is, as a window manager leaves it: taken up
            // from GPUI's window to the browser window's own, the keys go to neither.
            let focus = connection.get_input_focus().ok()?.reply().ok()?.focus;
            if !within(connection, focus, browser.window) {
                // Asked of the window manager, as a taskbar asks, so it knows which window has the
                // keyboard: one that thought another still had it would take that window's own
                // request to be activated, as Chrome's DevTools window makes, for done (#1039).
                let data = [2, x11rb::CURRENT_TIME, 0, 0, 0];
                let active = atom("_NET_ACTIVE_WINDOW")?;
                let message = xproto::ClientMessageEvent::new(32, browser.window, active, data);
                let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
                connection.send_event(false, *root, mask, message).ok()?;
                ACTIVATING.set(Some(browser.window));
            }
            connection.flush().ok()
        }
        "click" | "rightclick" | "middleclick" | "doubleclick" => {
            let (x, y) = point(&mut words)?;
            let at = target_at(x, y)?;
            let which = match action {
                "rightclick" => 3,
                "middleclick" => 2,
                _ => 1,
            };
            click(at, which, click_modifiers(words), false)?;
            if action == "doubleclick" {
                click(at, 1, none, true)?;
            }
            Some(())
        }
        // sideclick <x> <y> back|forward: a mouse's side button, X's buttons 8 and 9.
        "sideclick" => {
            let (x, y) = point(&mut words)?;
            let at = target_at(x, y)?;
            let which = match words.next()? {
                "back" => 8,
                "forward" => 9,
                _ => return None,
            };
            click(at, which, none, false)
        }
        // handclick <x> <y>: a click there with the display's own pointer, as click's is now; the
        // host hears the screen's presses (page_presses.rs). The pointer stays there.
        "handclick" => {
            let (x, y) = point(&mut words)?;
            let at = target_at(x, y)?;
            hover(at)?;
            xtest_event(xproto::BUTTON_PRESS_EVENT, 1, at.root_x, at.root_y)?;
            xtest_event(xproto::BUTTON_RELEASE_EVENT, 1, at.root_x, at.root_y)
        }
        "clickkey" | "rightclickkey" | "doubleclickkey" => {
            let (x, y) = point(&mut words)?;
            // Settings' close button is the window manager's off the Mac, in its title bar.
            if action == "clickkey"
                && x.max(y) < CLOSE_CORNER
                && let Some(Keyboard::Settings(window, _)) = keyboard_window()
            {
                return close(window);
            }
            let at = target_in_key(x, y)?;
            click(
                at,
                if action == "rightclickkey" { 3 } else { 1 },
                none,
                false,
            )?;
            if action == "doubleclickkey" {
                click(at, 1, none, true)?;
            }
            Some(())
        }
        "dragover" | "dragdrop" => {
            let (x, y) = point(&mut words)?;
            // What it carries: its kind, then the rest of the line, which can hold spaces.
            let carried = words.next().map(|kind| {
                let value = line.splitn(5, ' ').nth(4).unwrap_or("");
                (kind, value)
            });
            drag_over(x, y, carried, action == "dragdrop")
        }
        "dragleave" => {
            drag_leave();
            Some(())
        }
        "dragkey" => {
            let (x, y) = point(&mut words)?;
            let (x2, y2) = point(&mut words)?;
            let at = target_in_key(x, y)?;
            let to = target_in_key(x2, y2)?;
            first_press(at, 1, false);
            button(at, 1, true, none)?;
            // Halfway first, so the view sees the drag start before it ends.
            motion(target_in_key((x + x2) / 2., (y + y2) / 2.)?, button_mask(1))?;
            motion(to, button_mask(1))?;
            button(to, 1, false, button_mask(1))
        }
        "drag" => {
            let (x, y) = point(&mut words)?;
            let (x2, y2) = point(&mut words)?;
            let at = target_at(x, y)?;
            let to = Target {
                window: at.window,
                ..target_in(driven()?.window, x2, y2)?
            };
            let to = relative_to(to, at.window)?;
            first_press(at, 1, false);
            button(at, 1, true, none)?;
            motion(to, button_mask(1))?;
            button(to, 1, false, button_mask(1))
        }
        "press" | "dragto" | "release" | "rightpress" | "rightdragto" | "rightrelease" => {
            let (x, y) = point(&mut words)?;
            let (which, step) = match action.strip_prefix("right") {
                Some(step) => (3, step),
                None => (1, action),
            };
            // The keys named after the point, such as ⌥ for a drag that drops a copy.
            let keys = click_modifiers(words);
            let held = button_mask(which) | keys;
            if step == "press" {
                let at = target_at(x, y)?;
                PRESSED.set(Some(at));
                first_press(at, which, false);
                return button(at, which, true, none | keys);
            }
            let to = target_in(driven()?.window, x, y)?;
            let to = match PRESSED.get() {
                Some(pressed) => relative_to(to, pressed.window)?,
                None => target_at(x, y)?,
            };
            if step == "dragto" {
                return motion(to, held);
            }
            PRESSED.set(None);
            button(to, which, false, held)
        }
        "move" => {
            let (x, y) = point(&mut words)?;
            motion(target_at(x, y)?, none)
        }
        // The display's own pointer, to the point, as a capture with the pointer drawn needs: the
        // window under it shows its cursor there. Without a point it goes back to the corner.
        "warp" => {
            let connection = server()?;
            let (connection, root) = &*connection;
            let (x, y) = match point(&mut words) {
                Some((x, y)) => {
                    let at = target_in(driven()?.window, x, y)?;
                    (at.root_x, at.root_y)
                }
                None => {
                    let screen = connection.setup().roots.iter().find(|s| s.root == *root)?;
                    (
                        screen.width_in_pixels as i16 - 1,
                        screen.height_in_pixels as i16 - 1,
                    )
                }
            };
            connection
                .warp_pointer(x11rb::NONE, *root, 0, 0, 0, 0, x, y)
                .ok()?;
            connection.flush().ok()
        }
        // Each event moves the content up by <dy> points, as drive.mm's trackpad events do.
        // scrollkey scrolls in the window with the keyboard, such as Settings', as clickkey
        // clicks there.
        "scroll" | "scrollkey" => {
            let (x, y) = point(&mut words)?;
            let dy: f64 = words.next()?.parse().ok()?;
            let count: usize = words.next()?.parse().ok()?;
            let at = match action {
                "scrollkey" => target_in_key(x, y)?,
                _ => target_at(x, y)?,
            };
            // A page, Chromium's, scrolls a notch of the wheel an event, the notches alone: with a
            // move before them, the window run's page stopped short of its 3000 points under Xvfb.
            if at.window == driven()?.window {
                for _ in 0..count.max(1) {
                    click(at, if dy > 0. { 5 } else { 4 }, none, false)?;
                }
                return Some(());
            }
            // GPUI's windows scroll by the points asked, as the Mac's trackpad events do, where a
            // wheel's notch moves three lines.
            motion(at, none)?;
            let logical = |each: i16| px((f64::from(each) / scale()) as f32);
            let position = gpui::point(logical(at.x), logical(at.y));
            for _ in 0..count.max(1) {
                if !gpui_linux::drive_scroll(at.window, position, px(dy as f32)) {
                    click(at, if dy > 0. { 5 } else { 4 }, none, false)?;
                }
            }
            Some(())
        }
        "wheel" => {
            let (x, y) = point(&mut words)?;
            let dx: f64 = words.next()?.parse().ok()?;
            let at = target_at(x, y)?;
            // Lines right are the Mac's wheel's positive x, which GPUI reads from button 6 here.
            for _ in 0..(dx.abs().round() as usize).max(1) {
                click(at, if dx > 0. { 6 } else { 7 }, none, false)?;
            }
            Some(())
        }
        // Handed to the app as the Mac's driver hands it to its host's monitor's handler.
        // shortcut: no Linux input, such as a mouse's back and forward buttons, sends one yet.
        "pageswipe" => {
            let (x, y) = point(&mut words)?;
            let back = match words.next()? {
                "back" => true,
                "forward" => false,
                _ => return None,
            };
            crate::swiped(driven()?.ui as usize as *mut c_void, x, y, back);
            Some(())
        }
        "key" => {
            let name = words.next()?;
            let words: Vec<_> = words.collect();
            let mut state = modifiers(words.iter().copied());
            // ⌃Tab stays Ctrl+Tab, as keys.rs presses it, since Alt+Tab is the desktop's.
            let (ctrl, alt) = (u16::from(KeyButMask::CONTROL), u16::from(KeyButMask::MOD1));
            if name == "tab" && words.contains(&"ctrl") && !words.contains(&"cmd") {
                let option = if words.contains(&"alt") { alt } else { 0 };
                state = state & !alt | ctrl | option;
            }
            if words.contains(&"again") {
                let name = name.to_owned();
                press_again(Box::new(move || key(&name, state)));
            }
            key(name, state)
        }
        "keyonreturn" => {
            let key = format!("key {rest}");
            let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
            go_on(Box::new(move |done| key_on_return(key, until, done)));
            Some(())
        }
        // Each modifier going down or up as its key does, the left one, in the state held before
        // it, as the X server reports a key.
        // The modifiers the last key went with are held too, as the Mac's app takes them to be
        // until the flags change, such as ⌃ through a ⌃Tab walk.
        "flags" => {
            let to = modifiers(words);
            FLAGS.set(FLAGS.get() | KEY_FLAGS.take());
            // The keys `flags` holds go sent, with the lock, till the drive moves them: the lock
            // takes over a Control that XTEST holds, before XTEST lets it go.
            if FLAGS.get() != 0 {
                hold_modifiers()?;
            }
            let keys = [
                (KeyButMask::CONTROL, 0xffe3),
                (KeyButMask::MOD1, 0xffe9),
                (KeyButMask::SHIFT, 0xffe1),
            ];
            for (mask, keysym) in keys {
                let (mask, held) = (u16::from(mask), FLAGS.get());
                let kind = match (held & mask != 0, to & mask != 0) {
                    (false, true) => xproto::KEY_PRESS_EVENT,
                    (true, false) => xproto::KEY_RELEASE_EVENT,
                    _ => continue,
                };
                sent_code_events(keycode(keysym)?.0, held, &[kind])?;
                FLAGS.set(held ^ mask);
            }
            hold_modifiers();
            Some(())
        }
        // Repeats are presses like the first, so repeat changes nothing here.
        "keydown" => key_events(words.next()?, none, &[xproto::KEY_PRESS_EVENT]),
        "keyup" => key_events(words.next()?, none, &[xproto::KEY_RELEASE_EVENT]),
        "type" => rest.chars().try_for_each(|c| key(&c.to_string(), none)),
        "insert" => insert(rest),
        "ime" => {
            IME_ON.set(match words.next()? {
                "on" => true,
                "off" => false,
                _ => return None,
            });
            COMPOSITION.set(None);
            Some(())
        }
        // As the window manager's full-screen key, through the core, as the Mac's goes through the
        // green button.
        "fullscreen" => {
            unsafe { bc::bc_window_toggle_full_screen(driven_id()?) };
            Some(())
        }
        // As the window manager's maximize key: it maximizes the driven window, or restores it
        // once maximized, through the root window, as a pager asks (EWMH's _NET_WM_STATE). Chromium
        // hears of it after the window manager has done it, as of the user's (#1454).
        "maximize" => {
            let browser = driven()?;
            let maximized = maximized(browser.window)?;
            let x = server()?;
            let (connection, root) = &*x;
            let [state, vertical, horizontal] = [
                "_NET_WM_STATE",
                "_NET_WM_STATE_MAXIMIZED_VERT",
                "_NET_WM_STATE_MAXIMIZED_HORZ",
            ]
            .map(atom);
            // 0 takes the state off and 1 puts it on, and 1 says it's from an application.
            let data = [u32::from(!maximized), vertical?, horizontal?, 1, 0];
            let message = xproto::ClientMessageEvent::new(32, browser.window, state?, data);
            let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
            connection.send_event(false, *root, mask, message).ok()?;
            connection.flush().ok()
        }
        // As a pager's show-desktop button or openbox's Super+D, Linux's own: the window manager
        // hides every window to show the desktop, or shows them again, asked through the root
        // window (EWMH's _NET_SHOWING_DESKTOP), as wmctrl -k asks.
        "desktop" => {
            let on = match words.next()? {
                "on" => 1,
                "off" => 0,
                "shell" => return desktop_shell(words.next() != Some("off")),
                _ => return None,
            };
            let showing = atom("_NET_SHOWING_DESKTOP")?;
            let x = server()?;
            let (connection, root) = &*x;
            let message = xproto::ClientMessageEvent::new(32, *root, showing, [on, 0, 0, 0, 0]);
            let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
            connection.send_event(false, *root, mask, message).ok()?;
            connection.flush().ok()
        }
        // What the X server shows of the driven window and the app's windows over it, Linux's own:
        // "browser: onscreen window <0|1>, overlays <on screen> of <shown>, keyboard
        // <overlay|window|none>", the window that has the X input focus.
        "onscreen" => {
            let browser = driven()?;
            let x = server()?;
            let (connection, _) = &*x;
            let viewable = |window: xproto::Window| {
                connection
                    .get_window_attributes(window)
                    .ok()
                    .and_then(|cookie| cookie.reply().ok())
                    .is_some_and(|a| a.map_state == xproto::MapState::VIEWABLE)
            };
            let views = overlay_views();
            let on = (views.iter())
                .filter(|&&view| viewable(view as usize as u32))
                .count();
            let keyboard = match keyboard_window() {
                Some(Keyboard::Overlay(_)) => "overlay",
                Some(Keyboard::Browser(..) | Keyboard::Settings(..)) => "window",
                None => "none",
            };
            log!(
                "browser: onscreen window {}, overlays {on} of {}, keyboard {keyboard}",
                u8::from(viewable(browser.window)),
                views.len()
            );
            Some(())
        }
        // The driven window's state as the window manager has it and its frame now, as Windows'
        // driver logs its placement: "browser: placement <maximized|normal> <x> <y> <width>
        // <height>". The window manager keeps the frame a maximized window restores to itself.
        "placement" => {
            let browser = driven()?;
            let state = match maximized(browser.window)? {
                true => "maximized",
                false => "normal",
            };
            let [x, y, width, height] = frame(browser.window)?;
            log!("browser: placement {state} {x:.0} {y:.0} {width:.0} {height:.0}");
            Some(())
        }
        "resize" | "place" => {
            let browser = driven()?;
            let scale = scale();
            let numbers: Vec<f64> = words.map(|w| w.parse().ok()).collect::<Option<_>>()?;
            let aux = match (action, numbers.as_slice()) {
                ("resize", &[_, _]) => ConfigureWindowAux::new(),
                ("place", &[x, y, _, _]) => ConfigureWindowAux::new()
                    .x((x * scale) as i32)
                    .y((y * scale) as i32),
                _ => return None,
            };
            let [width, height] = [numbers[numbers.len() - 2], numbers[numbers.len() - 1]];
            let aux = aux
                .width((width * scale) as u32)
                .height((height * scale) as u32);
            let x = server()?;
            x.0.configure_window(browser.window, &aux).ok()?;
            // The window manager gives the window its size a moment later, and the UI follows,
            // where the Mac's resize is done at once (#1065).
            resizing(browser.gpui as usize, width, height);
            x.0.flush().ok()
        }
        // Settings with the keyboard, as resize does the driven browser window.
        "resizekey" => {
            let numbers: Vec<f64> = words.map(|w| w.parse().ok()).collect::<Option<_>>()?;
            let &[width, height] = numbers.as_slice() else {
                return None;
            };
            let Some(Keyboard::Settings(settings, _)) = keyboard_window() else {
                return None;
            };
            let scale = scale();
            let aux = ConfigureWindowAux::new()
                .width((width * scale) as u32)
                .height((height * scale) as u32);
            let x = server()?;
            x.0.configure_window(settings, &aux).ok()?;
            x.0.flush().ok()
        }
        "order" => {
            let stacked = stacked()?;
            let mut order = String::new();
            for browser in &stacked {
                let name = name_of(browser);
                order += &format!(" {name}");
                let [x, y, width, height] = frame(browser.window)?;
                log!("browser: frame {name} {x:.0} {y:.0} {width:.0} {height:.0}");
            }
            let key = match keyboard_window() {
                Some(Keyboard::Browser(browser, _)) => name_of(&browser),
                _ => "none".to_string(),
            };
            log!("browser: order{order}, key {key}");
            Some(())
        }
        "stack" => stack(),
        "front" => {
            let driven = driven().map(|b| b.id);
            let describe = |browser: Option<Browser>| match browser {
                Some(b) if Some(b.id) == driven => "driven".to_string(),
                Some(b) => name_of(&b),
                None => "none".to_string(),
            };
            let first = describe(stacked()?.first().copied());
            let key = match keyboard_window() {
                Some(Keyboard::Browser(browser, _)) => describe(Some(browser)),
                _ => "none".to_string(),
            };
            log!("browser: front {first}, main {key}, key {key}, ax focused {key}, ax main {key}");
            Some(())
        }
        // The window's content and the views the X server has in it: GPUI's, and the overlays
        // and floats over it, with a tab's dialog, which the core reports, as the Mac's lists its
        // child windows. The floats, such as the zoom HUD, take no input, so they're left out of
        // overlay_views. The pages are where the UI lays them out, and docked DevTools where the
        // core says (pages).
        "layout" => {
            let browser = driven()?;
            let [_, _, width, height] = frame(browser.window)?;
            let pages = pages(browser.id, width, height).unwrap_or_default();
            let ring = ring(browser.id, height).unwrap_or_default();
            log!(
                "browser: layout content {width:.0}x{height:.0}, GPUIView 0,0 {width:.0}x{height:.0}{pages}{ring}"
            );
            log!(
                "browser: layout buttons {}",
                buttons(browser.id).unwrap_or_default()
            );
            if let Some(dialog) = dialog(browser.id) {
                log!("browser: layout child {dialog}");
            }
            let [left, top, ..] = frame(browser.window)?;
            let views = overlay_views().into_iter().map(|view| view as usize as u32);
            for view in views.chain(popups(browser.window).unwrap_or_default()) {
                let [x, y, width, height] = frame(view)?;
                log!(
                    "browser: layout child {:.0},{:.0} {width:.0}x{height:.0}",
                    x - left,
                    y - top
                );
            }
            log!(
                "browser: layout keyboard {}",
                keyboard_page(browser.id, height)
            );
            log!(
                "browser: layout full screen {}",
                full_screen(browser.window)? as u8
            );
            Some(())
        }
        "state" => Some(()),
        "title" => {
            let title = key_title()?;
            log!("browser: key window titled {title}");
            Some(())
        }
        // The headless session's clipboard is the run's own, as drive.mm's pasteboard is. Under X
        // the driver asks for it on its own connection, and settle logs it.
        "pasteboard" => {
            if ask_clipboard().is_some() {
                return Some(());
            }
            let cx = CX.with_borrow(Clone::clone)?;
            let text = cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text()));
            log!("browser: pasteboard text {}", text.unwrap_or_default());
            Some(())
        }
        // The app's menus, which Linux shows no bar for, run in the driven window as the Mac's
        // menu bar runs them in the key one.
        "menu" | "menubar" => {
            let menus = MENUS.with_borrow(|menus| menus.build());
            if action == "menubar" {
                let menu = menus.iter().find(|menu| menu.name.as_ref() == rest)?;
                let mut titles = Vec::new();
                let mut add = |item: &MenuItem, prefix: &str| {
                    if let MenuItem::Action {
                        name,
                        checked,
                        disabled,
                        ..
                    } = item
                    {
                        let mut title = format!("{prefix}{name}");
                        title += if *checked { " (checked)" } else { "" };
                        title += if *disabled { " (disabled)" } else { "" };
                        titles.push(title);
                    }
                };
                for item in &menu.items {
                    match item {
                        MenuItem::Submenu(sub) => {
                            let prefix = format!("{} › ", sub.name);
                            sub.items.iter().for_each(|inner| add(inner, &prefix));
                        }
                        _ => add(item, ""),
                    }
                }
                log!("browser: menu bar {rest}: {}", titles.join(", "));
                return Some(());
            }
            let MenuItem::Action { action, .. } = menu_item(&menus, rest)? else {
                return None;
            };
            let handle = WINDOWS.with_borrow(|windows| {
                let id = driven_id()?;
                windows.iter().find(|w| w.id == id).map(|w| w.handle)
            })?;
            let cx = CX.with_borrow(Clone::clone)?;
            cx.update(|cx| {
                handle.update(cx, |_, window, cx| {
                    window.dispatch_action(action.boxed_clone(), cx)
                })
            })
            .ok()
        }
        // An item of ilo's menu that shows, a button of ilo's dialog, or an alert's button.
        "choose" => {
            let title = CString::new(rest).ok()?;
            let in_dialog = || {
                CX.with_borrow(Clone::clone)
                    .is_some_and(|cx| cx.update(|cx| browser::dialog::choose(rest, cx)))
            };
            (menu_window::choose(title.as_ptr())
                || in_dialog()
                || unsafe { platform::host_choose(title.as_ptr()) })
            .then_some(())
        }
        "dismiss" => menu_window::dismiss().then_some(()),
        "agentspace" => {
            let agent = words.next()?;
            let name = rest.get(agent.len() + 1..)?.trim();
            (!name.is_empty()).then_some(())?;
            let (agent_c, name_c) = (CString::new(agent).ok()?, CString::new(name).ok()?);
            let space = unsafe {
                bc::bc_space_create_for_agent(name_c.as_ptr(), agent_c.as_ptr(), name_c.as_ptr())
            };
            log!("browser: made space {space} for agent {agent}");
            Some(())
        }
        "agentmayact" => {
            let agent = words.next()?;
            let tab: bc::bc_id = words.next()?.parse().ok()?;
            let what = words.next().unwrap_or("navigate");
            let (agent_c, what_c) = (CString::new(agent).ok()?, CString::new(what).ok()?);
            let mut reason = [0 as std::ffi::c_char; 256];
            let may = unsafe {
                bc::bc_agent_may_act(
                    agent_c.as_ptr(),
                    tab,
                    what_c.as_ptr(),
                    reason.as_mut_ptr(),
                    reason.len(),
                )
            };
            log!("browser: agent {agent} may act on {tab}: {}", may as u8);
            if !may {
                let reason = unsafe { std::ffi::CStr::from_ptr(reason.as_ptr()) }.to_string_lossy();
                log!("browser: agent {agent} refused {tab}: {reason}");
            }
            Some(())
        }
        "agentopen" => {
            let agent = words.next()?;
            let url = words.next()?;
            let (agent_c, url_c) = (CString::new(agent).ok()?, CString::new(url).ok()?);
            let tab = unsafe { bc::bc_agent_open_tab(agent_c.as_ptr(), url_c.as_ptr()) };
            (tab != 0).then_some(())?;
            log!("browser: agent {agent} opened {tab}");
            Some(())
        }
        "agentrelease" => {
            let agent = words.next()?;
            let tab: bc::bc_id = words.next()?.parse().ok()?;
            let agent_c = CString::new(agent).ok()?;
            let released = unsafe { bc::bc_agent_release_tab(agent_c.as_ptr(), tab) };
            log!("browser: agent {agent} released {tab}: {}", released as u8);
            Some(())
        }
        "agentdisconnect" => {
            let agent = words.next()?;
            let agent_c = CString::new(agent).ok()?;
            let ended = unsafe { bc::bc_agent_disconnected(agent_c.as_ptr()) };
            log!("browser: agent {agent} disconnected, {ended} sessions ended");
            Some(())
        }
        "takeback" => {
            let tab: bc::bc_id = words.next()?.parse().ok()?;
            let took = unsafe { bc::bc_tab_take_back(tab) };
            log!("browser: took back {tab}: {}", took as u8);
            Some(())
        }
        "age" => {
            let seconds: f64 = words.next()?.parse().ok()?;
            unsafe { bc::bc_age_tabs_for_testing(seconds) };
            Some(())
        }
        // battery <percent> plugged|unplugged: the check's battery, with --battery-for-testing.
        "battery" => {
            let percent: i32 = words.next()?.parse().ok()?;
            let plugged = match words.next()? {
                "plugged" => true,
                "unplugged" => false,
                _ => return None,
            };
            unsafe { bc::bc_battery_for_testing(percent, plugged) };
            Some(())
        }
        // pair <tab> pin|confirm|passkey: a Bluetooth device the tab's page pairs with asks.
        "pair" => {
            let tab: u64 = words.next()?.parse().ok()?;
            let kind = match words.next()? {
                "pin" => bc::BC_PAIR_PIN,
                "confirm" => bc::BC_PAIR_CONFIRM,
                "passkey" => bc::BC_PAIR_PASSKEY,
                _ => return None,
            };
            unsafe { bc::bc_tab_pair_for_testing(tab, kind as i32) };
            Some(())
        }
        "pip" => mini_player_command(words),
        _ => None,
    }
}

/// Chromium's Picture in Picture window, if one shows: a window of the X client's that made the
/// driven browser window, kept over the others, as Chrome keeps a video's, the Mini Player, and a
/// page's, as a call floats in. The page's is titled as its opener, so the title alone won't do.
fn mini_player() -> Option<xproto::Window> {
    let browser = driven()?;
    let x = server()?;
    let (connection, root) = &*x;
    let client = !connection.setup().resource_id_mask;
    let (state, above) = (atom("_NET_WM_STATE")?, atom("_NET_WM_STATE_ABOVE")?);
    let floats = |window: xproto::Window| {
        (connection
            .get_property(false, window, state, xproto::AtomEnum::ATOM, 0, 64)
            .ok())
        .and_then(|cookie| cookie.reply().ok())
        .and_then(|property| Some(property.value32()?.any(|each| each == above)))
        .unwrap_or(false)
    };
    let ours = |window: xproto::Window| {
        window & client == browser.window & client
            && ((text_property(window, "_NET_WM_NAME"))
                .is_some_and(|title| title.eq_ignore_ascii_case("Picture in Picture"))
                || floats(window))
            && (connection.get_window_attributes(window).ok())
                .and_then(|cookie| cookie.reply().ok())
                .is_some_and(|a| a.map_state == xproto::MapState::VIEWABLE)
    };
    let tops = connection.query_tree(*root).ok()?.reply().ok()?.children;
    // A window manager that reparents, as openbox does, holds it in a frame of its own.
    tops.into_iter().find_map(|top| {
        if ours(top) {
            return Some(top);
        }
        let children = connection.query_tree(top).ok()?.reply().ok()?.children;
        children.into_iter().find(|&child| ours(child))
    })
}

/// pip, and pip doubleclick, wheel, press, dragto and release (run_command's header).
fn mini_player_command<'a>(mut words: impl Iterator<Item = &'a str>) -> Option<()> {
    let Some(window) = mini_player() else {
        log!("browser: pip none");
        return Some(());
    };
    let action = words.next();
    let Some(action) = action else {
        let [x, y, width, height] = frame(window)?;
        log!("browser: pip at {x:.0} {y:.0} {width:.0} {height:.0}");
        watch_mini_player(window);
        return Some(());
    };
    let (x, y) = point(&mut words)?;
    if let "press" | "dragto" | "release" = action {
        return hold_mini_player(window, action, x, y);
    }
    let at = target_in(window, x, y)?;
    match action {
        "doubleclick" => {
            click(at, 1, 0, false)?;
            click(at, 1, 0, true)
        }
        // pip wheel <x> <y> <notches> [cmd]: a notch of the wheel for each, down for content moving
        // up, as scroll's.
        "wheel" => {
            let notches: i32 = words.next()?.parse().ok()?;
            let state = click_modifiers(words);
            for _ in 0..notches.unsigned_abs() {
                click(at, if notches > 0 { 5 } else { 4 }, state, false)?;
            }
            Some(())
        }
        _ => None,
    }
}

/// pip press, pip dragto and pip release <x> <y>, at points where the window was at the press: the
/// display's own pointer, through XTest. Sent events can't move it, as the window manager moves it
/// with the pointer the X server has, as a hand moves it.
fn hold_mini_player(window: xproto::Window, step: &str, x: f64, y: f64) -> Option<()> {
    let server = server()?;
    let (connection, root) = &*server;
    let fake = |kind: u8, button: u8, (x, y): (i16, i16)| xtest_event(kind, button, x, y);
    if step == "press" {
        let at = target_in(window, x, y)?;
        let was = connection.query_pointer(*root).ok()?.reply().ok()?;
        let back = (was.root_x, was.root_y);
        fake(xproto::MOTION_NOTIFY_EVENT, 0, (at.root_x, at.root_y))?;
        // The pointer is the display's, so a press anywhere but on the Mini Player would be
        // another window's.
        let under = connection.query_pointer(*root).ok()?.reply().ok()?.child;
        if !within(connection, window, under) {
            log!("browser: pip press: another window is over the Mini Player there");
            return fake(xproto::MOTION_NOTIFY_EVENT, 0, back);
        }
        first_press(at, 1, false);
        let held = HeldPip {
            x,
            y,
            root: (at.root_x, at.root_y),
            back,
            frame: frame(window)?,
        };
        PIP_HELD.set(Some(held));
        return fake(xproto::BUTTON_PRESS_EVENT, 1, (at.root_x, at.root_y));
    }
    let held = PIP_HELD.get()?;
    let scale = scale();
    let to = (
        held.root.0 + ((x - held.x) * scale).round() as i16,
        held.root.1 + ((y - held.y) * scale).round() as i16,
    );
    fake(xproto::MOTION_NOTIFY_EVENT, 0, to)?;
    if step == "dragto" {
        return Some(());
    }
    // The window manager takes the pointer only once it has the core's request to move the
    // window, which can be after the drag's last motion, and it then moves the window with the
    // pointer until a click, so the release would leave the window following the pointer home.
    // A hand that drags keeps moving: a pixel back and forth till the window has moved or sized,
    // or 2 s.
    if to != held.root {
        for _ in 0..20 {
            if frame(window)? != held.frame {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
            fake(xproto::MOTION_NOTIFY_EVENT, 0, (to.0 + 1, to.1))?;
            fake(xproto::MOTION_NOTIFY_EVENT, 0, to)?;
        }
    }
    PIP_HELD.set(None);
    fake(xproto::BUTTON_RELEASE_EVENT, 1, to)?;
    // A hand rests after it lets go. The window manager takes a move's queued motions as one, the
    // last first, so the pointer sent home before it has read the release took the window with it,
    // its grabbed point under the corner (#1780). The pointer waits till no one holds it, or 2 s.
    for _ in 0..20 {
        if !pointer_held_elsewhere()? {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // Where the drag left it, as the last frame the window manager gave it may have come before.
    let [left, top, width, height] = frame(window)?;
    log!("browser: pip at {left:.0} {top:.0} {width:.0} {height:.0}");
    fake(xproto::MOTION_NOTIFY_EVENT, 0, held.back)
}

/// Whether another client holds the display's pointer, as the window manager does through a move
/// or resize it took over, and the window a press was on until its release: a grab of ours fails
/// then. Ours, when it succeeds, goes at once.
fn pointer_held_elsewhere() -> Option<bool> {
    let server = server()?;
    let (connection, root) = &*server;
    let grab = connection.grab_pointer(
        false,
        *root,
        EventMask::NO_EVENT,
        xproto::GrabMode::ASYNC,
        xproto::GrabMode::ASYNC,
        x11rb::NONE,
        x11rb::NONE,
        x11rb::CURRENT_TIME,
    );
    let status = grab.ok()?.reply().ok()?.status;
    if status == xproto::GrabStatus::SUCCESS {
        connection.ungrab_pointer(x11rb::CURRENT_TIME).ok()?;
        connection.flush().ok()?;
    }
    Some(status == xproto::GrabStatus::ALREADY_GRABBED)
}

/// A press on the Mini Player through XTest: where it was in the window and on the screen, where
/// the display's pointer was before it, where the release puts it back, and the window's frame.
#[derive(Clone, Copy)]
struct HeldPip {
    x: f64,
    y: f64,
    root: (i16, i16),
    back: (i16, i16),
    frame: [f64; 4],
}

/// Logs the Mini Player's frame each time `window`, it, moves or resizes, from a thread of its own
/// on a connection of its own, as watch_zoom does.
fn watch_mini_player(window: xproto::Window) {
    static WATCHER: OnceLock<Option<Arc<RustConnection>>> = OnceLock::new();
    // The display's scale, which the thread can't ask the core for.
    static SCALE: AtomicU64 = AtomicU64::new(0);
    SCALE.store(scale().to_bits(), Ordering::Relaxed);
    let watcher = WATCHER.get_or_init(|| {
        let (connection, screen) = x11rb::connect(None).ok()?;
        let root = connection.setup().roots.get(screen)?.root;
        let connection = Arc::new(connection);
        let reader = connection.clone();
        std::thread::spawn(move || {
            while let Ok(event) = reader.wait_for_event() {
                match event {
                    // Placed in the root's terms, as the event places it only where no window
                    // manager reparents it, as none does in sway, but openbox does.
                    Event::ConfigureNotify(e) => {
                        let Some(at) = (reader.translate_coordinates(e.window, root, 0, 0).ok())
                            .and_then(|cookie| cookie.reply().ok())
                        else {
                            continue;
                        };
                        let scale = f64::from_bits(SCALE.load(Ordering::Relaxed));
                        let [x, y] = [at.dst_x, at.dst_y].map(|n| f64::from(n) / scale);
                        let [width, height] = [e.width, e.height].map(|n| f64::from(n) / scale);
                        log!("browser: pip at {x:.0} {y:.0} {width:.0} {height:.0}");
                    }
                    Event::UnmapNotify(_) | Event::DestroyNotify(_) => log!("browser: pip gone"),
                    _ => {}
                }
            }
        });
        Some(connection)
    });
    let Some(connection) = watcher else { return };
    let aux = ChangeWindowAttributesAux::new().event_mask(EventMask::STRUCTURE_NOTIFY);
    if let Ok(cookie) = connection.change_window_attributes(window, &aux) {
        cookie.ignore_error();
    }
    connection.flush().ok();
}

/// `to`, a point in the root's terms, in `window`, which a press grabbed the pointer for.
fn relative_to(to: Target, window: xproto::Window) -> Option<Target> {
    let x = server()?;
    let (connection, root) = &*x;
    let at = connection
        .translate_coordinates(*root, window, to.root_x, to.root_y)
        .ok()?
        .reply()
        .ok()?;
    Some(Target {
        window,
        x: at.dst_x,
        y: at.dst_y,
        ..to
    })
}

#[cfg(test)]
mod tests {
    use super::{xproto, xtest_delay, xtest_verdict};

    fn outputs(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    // A double click's two presses went out within a millisecond, so Chromium took the second
    // for the first handled again and the Mini Player saw no double click (#2721).
    #[test]
    fn xtest_presses_never_share_a_millisecond() {
        assert!(xtest_delay(xproto::BUTTON_PRESS_EVENT) >= 1);
        // Nothing else waits: a release, a move or a key goes as soon as it's sent.
        for kind in [
            xproto::BUTTON_RELEASE_EVENT,
            xproto::MOTION_NOTIFY_EVENT,
            xproto::KEY_PRESS_EVENT,
            xproto::KEY_RELEASE_EVENT,
        ] {
            assert_eq!(xtest_delay(kind), 0);
        }
    }

    #[test]
    fn xtest_drives_only_a_checks_own_display() {
        let headless = || Ok(());
        let desktop = || Err("sway's outputs aren't all headless".to_string());
        // Xvfb, as check.sh and CI start it, and headless.sh's Xwayland.
        assert!(xtest_verdict("xvfb", &outputs(&["screen"]), headless).is_ok());
        assert!(xtest_verdict("wayland-1", &outputs(&["HEADLESS-1"]), headless).is_ok());
        assert!(xtest_verdict("wayland-1", &outputs(&["XWAYLAND0"]), headless).is_ok());
        // Someone's own desktop: no BROWSER_HEADLESS, or one set by hand over a real screen.
        assert!(xtest_verdict("", &outputs(&["screen"]), headless).is_err());
        assert!(xtest_verdict("xvfb", &outputs(&["eDP-1", "HDMI-1"]), headless).is_err());
        assert!(xtest_verdict("xvfb", &outputs(&["XWAYLAND0"]), headless).is_err());
        assert!(xtest_verdict("xvfb", &[], headless).is_err());
        // A desktop sway's Xwayland, and an Xwayland that isn't the headless sway's.
        assert!(xtest_verdict("wayland-1", &outputs(&["XWAYLAND0"]), desktop).is_err());
        assert!(xtest_verdict("wayland-1", &outputs(&["DP-1"]), headless).is_err());
    }
}
