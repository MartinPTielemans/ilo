//! Keys: those a page's shortcut or the UI takes, those held for the UI, the input method's text,
//! and the Windows driver's input.

use super::*;

/// Gives the keyboard in `window` to the UI, whose GPUI view is `view`, to keep or, with
/// `until_page`, until a page shows. On Linux the core says whose the keys are, and takes them from
/// the page. On Windows it hears first, while the focus still says whether the UI had them.
pub(crate) fn focus_ui(window: bc::bc_id, view: *mut c_void, until_page: bool) {
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

/// A key the page didn't keep, or one it never sees, as a keystroke in GPUI's terms.
pub(crate) fn keystroke(key: &bc::bc_key) -> String {
    let text = |text: *const c_char| match text.is_null() {
        true => Default::default(),
        false => unsafe { CStr::from_ptr(text) }.to_string_lossy(),
    };
    shell::page_keystroke(&text(key.key), &text(key.latin_key), key.modifiers)
}

/// Runs the shortcut `key` is, if it's one of the page shortcuts that are `reserved` or aren't,
/// on the keys the user has now, in the window showing `tab`'s page, which had the key.
pub(crate) fn run_page_shortcut(tab: bc::bc_id, key: *const bc::bc_key, reserved: bool) -> bool {
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
    pub(crate) static SECOND_PRESS: RefCell<shell::SecondPress> = RefCell::default();
}

pub(crate) unsafe extern "C" fn tab_key_before_page(
    _: *mut c_void,
    tab: bc::bc_id,
    key: *const bc::bc_key,
) -> bool {
    // An agent's key comes from inside the Agents' update, where an update of ours would abort.
    if browser::agents::pressing_key() {
        return false;
    }
    let cx = CX.with_borrow(Clone::clone);
    shell::page_key_answer(cx.as_ref(), || {
        let (kept, route) = key_before_page(tab, key);
        log_key(key, "before the page", route);
        // Under Wayland GPUI reads this press next, and with the UI given the keyboard by the
        // answer, as when ⌘W closed the tab that had it, it would run the shortcut again (#2442).
        #[cfg(target_os = "linux")]
        if kept {
            gpui_linux::answered_key(&held_keystroke(unsafe { &*key }));
        }
        #[cfg(target_os = "linux")]
        drive::page_key_answered();
        kept
    })
}

/// The keystroke GPUI has for `key`, one typed in a page.
#[cfg(target_os = "linux")]
pub(crate) fn held_keystroke(key: &bc::bc_key) -> gpui::Keystroke {
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
pub(crate) fn give_held_keys(view: *mut c_void) {
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
    // It names the window that took them as the Mac's does.
    log!("browser: held {} key events for GPUIPanel", held.len());
    // After the update that showed the overlay, which a key would come inside, and before GPUI
    // reads the keys typed since, as the task runs before it dispatches its Wayland queue.
    let app = cx.clone();
    cx.foreground_executor()
        .spawn(async move {
            app.update(|cx| {
                let overlay = cx.windows().into_iter().find(|window| {
                    window
                        .update(cx, |_, window, _| ns_view(window) == view)
                        .unwrap_or(false)
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
pub(crate) unsafe extern "C" fn held_keys_unclaimed(_: *mut c_void) {
    unsafe { platform::host_release_held_keys() };
}

pub(crate) unsafe extern "C" fn tab_key_unhandled(
    _: *mut c_void,
    tab: bc::bc_id,
    key: *const bc::bc_key,
) -> bool {
    if browser::agents::pressing_key() {
        return false;
    }
    let cx = CX.with_borrow(Clone::clone);
    shell::page_key_answer(cx.as_ref(), || {
        SECOND_PRESS.with_borrow_mut(|second| second.declined(&keystroke(unsafe { &*key })));
        let (taken, route) = match link_preview_key(tab, key, false) {
            Some(lost) => (lost, "link preview"),
            None if run_page_shortcut(tab, key, false) => (true, "shortcut"),
            None => (false, "nothing"),
        };
        log_key(key, "the page didn't keep", route);
        taken
    })
}

/// Under Wayland the window's input method composes `text` in the UI's text field that has the keys.
#[cfg(target_os = "linux")]
pub(crate) unsafe extern "C" fn ui_composed(
    _: *mut c_void,
    window: bc::bc_id,
    text: *const c_char,
) {
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    if let Some(ui_view) = ui_view_of(window) {
        gpui_linux::compose_embedded(ui_view, &text);
    }
}

/// As `ui_composed`: the input method commits `text`.
#[cfg(target_os = "linux")]
pub(crate) unsafe extern "C" fn ui_committed(
    _: *mut c_void,
    window: bc::bc_id,
    text: *const c_char,
) {
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    if let Some(ui_view) = ui_view_of(window) {
        gpui_linux::commit_embedded(ui_view, &text);
    }
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

/// The Windows driver's two fingers (platform/windows/drive.cc), in GPUI's window `hwnd`: at `x`,
/// `y` in its client area's pixels, moving the content `dx` points right and `dy` down, as a
/// precision touchpad's come through Direct Manipulation: touching for `phase` 0, moving for 1 and
/// lifting for 2. Returns whether the window was GPUI's.
#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
pub extern "C" fn browser_drive_fingers(
    hwnd: *mut c_void,
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    phase: c_int,
) -> bool {
    let phase = match phase {
        0 => gpui::TouchPhase::Started,
        1 => gpui::TouchPhase::Moved,
        _ => gpui::TouchPhase::Ended,
    };
    gpui_windows::drive_fingers(hwnd, x, y, gpui::point(gpui::px(dx), gpui::px(dy)), phase)
}
